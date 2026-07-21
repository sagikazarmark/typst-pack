use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use graphql_parser::schema::{Definition, Document, Field, Type, TypeDefinition, parse_schema};
use serde::Deserialize;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value, json};
use typst_pack_interface::transport::prototype_validation::{
    FrozenTransportSubject, object_count as public_transport_object_count,
};

type CheckResult<T> = Result<T, String>;

const SCHEMA_FILE: &str = "PROTOTYPE-first-party-cli-dagger-schemas.json";
const NATIVE_PROFILE_FILE: &str = "PROTOTYPE-native-cli-profile.json";
const DAGGER_PROFILE_FILE: &str = "PROTOTYPE-dagger-ci-profile.json";
const GRAPHQL_FILE: &str = "PROTOTYPE-first-party-cli-dagger-generated.graphql";
const HTML_FILE: &str = "PROTOTYPE-first-party-cli-dagger-contracts.html";
const SERIALIZER_PROBE_FILE: &str = "PROTOTYPE-first-party-cli-dagger-serializer-probe.rs";
const RUST_INTERFACE_FILE: &str = "PROTOTYPE-rust-lifecycle-adapter-interfaces.rs";
const RUST_CONSUMER_FILE: &str = "PROTOTYPE-rust-lifecycle-adapter-consumer.rs";

#[derive(Clone)]
struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = UniqueValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueValue)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(UniqueValue(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(UniqueValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate JSON key `{key}`")));
            }
            let UniqueValue(value) = object.next_value()?;
            values.insert(key, value);
        }
        Ok(UniqueValue(Value::Object(values)))
    }
}

#[derive(Default)]
struct Summary {
    definitions: usize,
    local_refs: usize,
    schema_cases: usize,
    generated_cases: usize,
    semantic_cases: usize,
    capability_constants: usize,
    graphql_types: usize,
    scenarios: usize,
    allowed_events: usize,
    covered_events: usize,
    allowed_effects: usize,
    publishing_sequences: usize,
    delivery_wrappers: usize,
    source_leaves: usize,
    poison_cases: usize,
}

#[derive(Default)]
struct ReplayOccupancyLedger {
    backings: BTreeMap<&'static str, (u64, u64, u64)>,
    live_spool: u64,
    live_memory: u64,
    peak_spool: u64,
    peak_memory: u64,
}

struct ReplayOccupancyReservation {
    backing: &'static str,
}

impl ReplayOccupancyLedger {
    fn reserve(
        &mut self,
        backing: &'static str,
        spool: u64,
        memory: u64,
    ) -> CheckResult<ReplayOccupancyReservation> {
        if let Some((existing_spool, existing_memory, references)) = self.backings.get_mut(backing)
        {
            if (*existing_spool, *existing_memory) != (spool, memory) {
                return Err("shared backing changed its accounting dimensions".to_owned());
            }
            *references += 1;
        } else {
            self.live_spool = self.live_spool.checked_add(spool).ok_or("spool overflow")?;
            self.live_memory = self
                .live_memory
                .checked_add(memory)
                .ok_or("memory overflow")?;
            self.peak_spool = self.peak_spool.max(self.live_spool);
            self.peak_memory = self.peak_memory.max(self.live_memory);
            self.backings.insert(backing, (spool, memory, 1));
        }
        Ok(ReplayOccupancyReservation { backing })
    }

    fn transfer(&mut self, reservation: ReplayOccupancyReservation) -> ReplayOccupancyReservation {
        reservation
    }

    fn release(&mut self, reservation: ReplayOccupancyReservation) -> CheckResult<()> {
        let Some((spool, memory, references)) = self.backings.get_mut(reservation.backing) else {
            return Err("released unknown backing".to_owned());
        };
        *references -= 1;
        if *references == 0 {
            let (spool, memory, _) = self
                .backings
                .remove(reservation.backing)
                .ok_or("lost backing during release")?;
            self.live_spool -= spool;
            self.live_memory -= memory;
        } else {
            let _ = (spool, memory);
        }
        Ok(())
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("final contract validation: FAILED: {error}");
        std::process::exit(1);
    }
}

fn run() -> CheckResult<()> {
    let schema = load_json(&parent_path(SCHEMA_FILE))?;
    let native = load_json(&parent_path(NATIVE_PROFILE_FILE))?;
    let dagger = load_json(&parent_path(DAGGER_PROFILE_FILE))?;
    let cases = load_json(&manifest_path("fixtures/cases.json"))?;
    assert_json_parser_guards()?;

    let mut summary = Summary::default();
    validate_schema_bundle(&schema, &mut summary)?;
    validate_corrected_schema_contract(&schema)?;
    validate_profiles(&schema, &native, &dagger, &cases)?;
    validate_capability_constants(&schema, &cases, &mut summary)?;
    validate_schema_cases(&schema, &native, &dagger, &cases, &mut summary)?;
    validate_semantics(&schema, &cases, &native, &dagger, &mut summary)?;

    let graphql = read_utf8(&parent_path(GRAPHQL_FILE))?;
    validate_graphql(&graphql, &mut summary)?;
    let html = read_utf8(&parent_path(HTML_FILE))?;
    validate_html(&html, &cases, &mut summary)?;
    let serializer_probe = read_utf8(&parent_path(SERIALIZER_PROBE_FILE))?;
    validate_source_manifest(&cases, &schema, &serializer_probe, &mut summary)?;
    validate_creation_serializer_coverage(&serializer_probe)?;
    let rust_interface = read_utf8(&parent_path(RUST_INTERFACE_FILE))?;
    let rust_consumer = read_utf8(&parent_path(RUST_CONSUMER_FILE))?;
    validate_corrected_rust_contract(&rust_interface, &rust_consumer)?;
    validate_session_admission_vectors()?;

    println!("final contract validation: ok");
    println!(
        "json-schema: Draft 2020-12, {} definitions, {} local refs, {} direct + {} generated cases",
        summary.definitions, summary.local_refs, summary.schema_cases, summary.generated_cases
    );
    println!(
        "profiles: 2 valid; final aggregate, representation, and transport relationships verified"
    );
    println!(
        "capabilities: {} producer-correlated constants and first-party trust/cache constraints verified",
        summary.capability_constants
    );
    println!(
        "graphql: {} types; hand-authored target topology, statuses, and nullability verified",
        summary.graphql_types
    );
    println!(
        "html: {} scenarios; {}/{} allowed events covered, {} allowed effects, {} publish fences and {} delivery wrappers structurally verified",
        summary.scenarios,
        summary.covered_events,
        summary.allowed_events,
        summary.allowed_effects,
        summary.publishing_sequences,
        summary.delivery_wrappers
    );
    println!(
        "serializer sources: {} mechanically linked leaves; {} poison derivations declared; {} semantic cases checked",
        summary.source_leaves, summary.poison_cases, summary.semantic_cases
    );
    println!("dagger generated parity: DEFERRED (implementation gate not executed)");
    Ok(())
}

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn parent_path(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(file)
}

fn read_utf8(path: &Path) -> CheckResult<String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(format!("{}: UTF-8 BOM is forbidden", path.display()));
    }
    String::from_utf8(bytes).map_err(|error| format!("{}: invalid UTF-8: {error}", path.display()))
}

fn load_json(path: &Path) -> CheckResult<Value> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    parse_json_bytes(&bytes, &path.display().to_string(), true)
}

fn parse_json_bytes(bytes: &[u8], label: &str, require_newline: bool) -> CheckResult<Value> {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(format!("{label}: UTF-8 BOM is forbidden"));
    }
    let body = if require_newline {
        bytes
            .strip_suffix(b"\n")
            .ok_or_else(|| format!("{label}: JSON document must end with exactly one newline"))?
    } else {
        bytes
    };
    if body.last().is_some_and(u8::is_ascii_whitespace) {
        return Err(format!(
            "{label}: whitespace before the final newline is forbidden"
        ));
    }
    let text =
        std::str::from_utf8(body).map_err(|error| format!("{label}: invalid UTF-8: {error}"))?;
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let UniqueValue(value) =
        UniqueValue::deserialize(&mut deserializer).map_err(|error| format!("{label}: {error}"))?;
    deserializer
        .end()
        .map_err(|error| format!("{label}: trailing bytes after JSON value: {error}"))?;
    Ok(value)
}

fn assert_json_parser_guards() -> CheckResult<()> {
    for (label, bytes) in [
        ("duplicate-key guard", b"{\"x\":1,\"x\":2}\n".as_slice()),
        ("BOM guard", b"\xef\xbb\xbf{}\n".as_slice()),
        ("trailing-bytes guard", b"{}\n{}\n".as_slice()),
    ] {
        if parse_json_bytes(bytes, label, true).is_ok() {
            return Err(format!("{label} accepted its poison document"));
        }
    }
    Ok(())
}

fn validate_schema_bundle(schema: &Value, summary: &mut Summary) -> CheckResult<()> {
    check_eq(
        schema.pointer("/$schema").and_then(Value::as_str),
        Some("https://json-schema.org/draft/2020-12/schema"),
        "schema draft",
    )?;
    jsonschema::draft202012::meta::validate(schema).map_err(|error| {
        format!("schema does not satisfy the Draft 2020-12 meta-schema: {error}")
    })?;
    jsonschema::draft202012::new(schema)
        .map_err(|error| format!("compile root Draft 2020-12 schema: {error}"))?;
    jsonschema::validator_map_for(schema)
        .map_err(|error| format!("compile every addressable schema location: {error}"))?;

    let definitions = object_at(schema, "/$defs")?;
    summary.definitions = definitions.len();
    let important = [
        "adapter_jobs",
        "unadmitted_adapter_jobs",
        "admitted_adapter_jobs",
        "adapter_resource_profile",
        "creation_request_rejection",
        "creation_admission_refusal",
        "create_operation",
        "compilation_terminal",
        "compile_operation",
        "format_receipt_common",
        "format_receipt",
        "pack_archive_encoding_report",
        "project_materialization_projection_receipt",
        "project_materialization_operation",
        "transport_stage_ledger",
        "transport_receipt",
        "session_publication",
        "session_state_projection",
        "watch_state",
    ];
    for name in important {
        if !definitions.contains_key(name) {
            return Err(format!(
                "schema is missing corrected root definition `{name}`"
            ));
        }
    }
    let archive_report_required =
        strings_at(schema, "/$defs/pack_archive_encoding_report/required")?;
    if archive_report_required != ["receipt", "spool_receipt"] {
        return Err(format!(
            "pack_archive_encoding_report must independently require receipt and spool_receipt, got {archive_report_required:?}"
        ));
    }

    inspect_schema_node(schema, schema, "", summary)?;
    Ok(())
}

fn inspect_schema_node(
    root: &Value,
    node: &Value,
    path: &str,
    summary: &mut Summary,
) -> CheckResult<()> {
    match node {
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("object") {
                let closed = object.get("additionalProperties") == Some(&Value::Bool(false))
                    || object.get("unevaluatedProperties") == Some(&Value::Bool(false));
                if !closed {
                    return Err(format!("schema object at {path} is not closed"));
                }
            }
            for keyword in ["required", "enum"] {
                if let Some(values) = object.get(keyword).and_then(Value::as_array) {
                    let mut unique = BTreeSet::new();
                    for value in values {
                        let encoded =
                            serde_json::to_string(value).map_err(|error| error.to_string())?;
                        if !unique.insert(encoded.clone()) {
                            return Err(format!("duplicate {keyword} value {encoded} at {path}"));
                        }
                    }
                }
            }
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                summary.local_refs += 1;
                if !reference.starts_with("#/") {
                    return Err(format!("non-local $ref `{reference}` at {path}"));
                }
                let pointer = &reference[1..];
                if root.pointer(pointer).is_none() {
                    return Err(format!("unresolved local $ref `{reference}` at {path}"));
                }
            }
            for (key, child) in object {
                inspect_schema_node(root, child, &format!("{path}/{key}"), summary)?;
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                inspect_schema_node(root, child, &format!("{path}/{index}"), summary)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_profiles(
    schema: &Value,
    native: &Value,
    dagger: &Value,
    cases: &Value,
) -> CheckResult<()> {
    validate_definition(
        schema,
        "adapter_resource_profile",
        native,
        true,
        "native profile",
    )?;
    validate_definition(
        schema,
        "adapter_resource_profile",
        dagger,
        true,
        "dagger profile",
    )?;
    assert_canonical_numeric_strings(native, "native profile")?;
    assert_canonical_numeric_strings(dagger, "dagger profile")?;
    check_eq(
        native.pointer("/profile/name").and_then(Value::as_str),
        Some("native-cli"),
        "native profile name",
    )?;
    check_eq(
        dagger.pointer("/profile/name").and_then(Value::as_str),
        Some("dagger-ci"),
        "Dagger profile name",
    )?;

    for (profile, expected) in [
        (
            native,
            [
                ("/creation/package_files", "100000"),
                ("/creation/largest_package_file_bytes", "536870912"),
                ("/creation/font_candidates", "8192"),
                ("/execution/creation/isolated_worker_capacity", "1"),
                ("/creation/aggregate_file_bindings", "100000"),
                ("/creation/aggregate_logical_bytes", "4294967296"),
                (
                    "/compilation_preparation/limits/diagnostic_entries",
                    "20000",
                ),
                (
                    "/compilation_preparation/limits/diagnostic_entry_bytes",
                    "67108864",
                ),
                ("/pack_ingress/logical_file_bindings", "100000"),
                ("/pack_ingress/logical_decoded_bytes", "4294967296"),
                ("/pack_ingress/physical_blob_bytes", "4294967296"),
                ("/pack_ingress/representation_entries", "200000"),
                ("/pack_ingress/closure_export_payload_bytes", "4362076160"),
                ("/representation/pack_archive/output_bytes", "1073741824"),
                ("/representation/closure_export/payload_bytes", "4362076160"),
                ("/representation/project_materialization/files", "100000"),
                (
                    "/representation/project_materialization/output_bytes",
                    "8589934592",
                ),
                ("/transport/objects", "200000"),
            ],
        ),
        (
            dagger,
            [
                ("/creation/package_files", "100000"),
                ("/creation/largest_package_file_bytes", "536870912"),
                ("/creation/font_candidates", "16384"),
                ("/execution/creation/isolated_worker_capacity", "2"),
                ("/creation/aggregate_file_bindings", "100000"),
                ("/creation/aggregate_logical_bytes", "8589934592"),
                (
                    "/compilation_preparation/limits/diagnostic_entries",
                    "20000",
                ),
                (
                    "/compilation_preparation/limits/diagnostic_entry_bytes",
                    "67108864",
                ),
                ("/pack_ingress/logical_file_bindings", "100000"),
                ("/pack_ingress/logical_decoded_bytes", "8589934592"),
                ("/pack_ingress/physical_blob_bytes", "8589934592"),
                ("/pack_ingress/representation_entries", "200000"),
                ("/pack_ingress/closure_export_payload_bytes", "8657043456"),
                ("/representation/pack_archive/output_bytes", "2147483648"),
                ("/representation/closure_export/payload_bytes", "8657043456"),
                ("/representation/project_materialization/files", "100000"),
                (
                    "/representation/project_materialization/output_bytes",
                    "17179869184",
                ),
                ("/transport/objects", "200000"),
            ],
        ),
    ] {
        for (pointer, value) in expected {
            check_eq(
                profile.pointer(pointer).and_then(Value::as_str),
                Some(value),
                &format!("profile field {pointer}"),
            )?;
        }
    }
    compare_profile_ceilings(native, dagger, "")?;
    validate_profile_coherence(native, "native-cli/1")?;
    validate_profile_coherence(dagger, "dagger-ci/1")?;
    validate_role_limit_projection(native, "native-cli/1")?;
    validate_role_limit_projection(dagger, "dagger-ci/1")?;
    let mut incoherent = native.clone();
    incoherent["transport"]["objects"] = json!("199999");
    if validate_profile_coherence(&incoherent, "incoherent-profile-poison").is_ok() {
        return Err("cross-field-incoherent profile was accepted".to_owned());
    }
    validate_resource_accounting_vectors()?;
    validate_all_stored_vectors()?;

    for overlay in array_at(cases, "/profile_tightening_overlays")? {
        let id = string_field(overlay, "id")?;
        let profile = match string_field(overlay, "profile")? {
            "native-cli" => native,
            "dagger-ci" => dagger,
            other => return Err(format!("overlay {id}: unknown profile `{other}`")),
        };
        let pointer = string_field(overlay, "pointer")?;
        let ceiling = profile
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("overlay {id}: missing numeric profile field {pointer}"))?;
        let value = string_field(overlay, "value")?;
        let valid = parse_decimal(value, &format!("overlay {id}"))?
            <= parse_decimal(ceiling, &format!("overlay {id} ceiling"))?;
        check_eq(
            Some(valid),
            overlay.get("valid").and_then(Value::as_bool),
            &format!("overlay {id} tightening result"),
        )?;
    }
    Ok(())
}

fn validate_corrected_schema_contract(schema: &Value) -> CheckResult<()> {
    let reached = strings_at(
        schema,
        "/$defs/creation_operational_inventory/properties/resources/properties/reached/required",
    )?
    .into_iter()
    .collect::<BTreeSet<_>>();
    let expected = [
        "project_files",
        "aggregate_project_bytes",
        "largest_project_file_bytes",
        "packages",
        "package_files",
        "largest_package_file_bytes",
        "package_tree_bytes",
        "font_containers",
        "font_candidates",
        "font_faces",
        "font_bytes",
        "discovery_variants",
        "discovery_restarts",
        "aggregate_file_bindings",
        "aggregate_logical_bytes",
        "override_count",
        "largest_override_bytes",
        "aggregate_override_bytes",
        "peak_stable_spool_bytes",
        "peak_retained_memory_bytes",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if reached != expected {
        return Err(format!(
            "creation reached-resource schema is not lossless: expected {expected:?}, got {reached:?}"
        ));
    }

    let session_preparation = schema
        .pointer("/$defs/session_policy/properties/preparation/oneOf")
        .and_then(Value::as_array)
        .ok_or("session policy preparation is not a discriminated exact contract")?;
    if session_preparation.len() != 2
        || session_preparation.iter().any(|branch| {
            branch
                .pointer("/properties/policy/$ref")
                .and_then(Value::as_str)
                != Some("#/$defs/compilation_preparation_policy")
                || branch
                    .pointer("/properties/limits/$ref")
                    .and_then(Value::as_str)
                    != Some("#/$defs/compilation_preparation_limits")
        })
    {
        return Err(
            "session policy does not reuse the exact one-shot preparation contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_corrected_rust_contract(interface: &str, consumer: &str) -> CheckResult<()> {
    for required in [
        "pub struct SessionPreparation {",
        "pub fn preparation_policy(",
        "pub fn preparation_limits(",
        "pub fn token(&self) -> &SessionAttemptToken",
        "pub fn try_admit_sync<'a",
        "pub fn try_admit_async<'a",
        "pub struct SessionAttemptAdmissionRefusal",
        "pub struct SessionAttemptCompletion",
        "pub struct AdmittedSyncSessionAttempt",
        "pub struct AdmittedAsyncSessionAttempt",
        "AttemptAdmissionRefused(SessionAttemptAdmissionRefusal)",
        "AttemptFinished(SessionAttemptCompletion)",
        "pub struct CompilationOperationAdmissionRecordView",
    ] {
        if !interface.contains(required) {
            return Err(format!("corrected Rust interface is missing `{required}`"));
        }
    }
    for forbidden in [
        "SessionPreparationLimits",
        "bind_session(",
        "StartAttempt {\n            token:",
        "pub fn prepared(&self) -> &crate::PreparedCompilation",
        "pub fn run_sync<P: ?Sized, F: ?Sized, C: ?Sized>(\n            self,\n            _controls:",
    ] {
        if interface.contains(forbidden) {
            return Err(format!(
                "corrected Rust interface retains forbidden `{forbidden}`"
            ));
        }
    }

    validate_session_rust_ast(interface)?;
    for required in [
        "SessionPreparation::caller_selected",
        "plan.try_admit_sync(",
        "plan.try_admit_async(",
        "admitted.operation_admission()",
        "let completion = admitted.run_sync()",
        "let completion = admitted.run_async().await",
        "completion.token()",
        "completion.report()",
    ] {
        if !consumer.contains(required) {
            return Err(format!("external consumer does not prove `{required}`"));
        }
    }
    Ok(())
}

#[derive(Default)]
struct SessionRustAst {
    plan_impls: usize,
    plan_methods: BTreeSet<String>,
    completion_impls: usize,
    completion_methods: BTreeSet<String>,
    completion_producers: BTreeSet<String>,
    protected_aliases: Vec<String>,
    protected_trait_impls: Vec<String>,
    session_macros: Vec<String>,
}

fn validate_session_rust_ast(interface: &str) -> CheckResult<()> {
    let file = syn::parse_file(interface)
        .map_err(|error| format!("cannot parse corrected Rust interface: {error}"))?;
    let mut audit = SessionRustAst::default();
    inspect_rust_items(&file.items, &mut audit, false);

    let expected_plan_methods = [
        "token",
        "revision",
        "evaluation",
        "policy",
        "prepared_identity",
        "supersession_permit",
        "try_admit_sync",
        "try_admit_async",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    if audit.plan_impls != 1 || audit.plan_methods != expected_plan_methods {
        return Err(format!(
            "SessionAttemptPlan interface differs: impls {}, methods {:?}",
            audit.plan_impls, audit.plan_methods
        ));
    }

    let expected_completion_methods = ["token", "report", "into_parts"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if audit.completion_impls != 1 || audit.completion_methods != expected_completion_methods {
        return Err(format!(
            "SessionAttemptCompletion interface differs: impls {}, methods {:?}",
            audit.completion_impls, audit.completion_methods
        ));
    }

    let expected_producers = [
        "AdmittedAsyncSessionAttempt::run_async",
        "AdmittedSyncSessionAttempt::run_sync",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    if audit.completion_producers != expected_producers {
        return Err(format!(
            "SessionAttemptCompletion producers differ: expected {expected_producers:?}, got {:?}",
            audit.completion_producers
        ));
    }
    if !audit.protected_aliases.is_empty() {
        return Err(format!(
            "protected session types have aliases: {:?}",
            audit.protected_aliases
        ));
    }
    if !audit.protected_trait_impls.is_empty() {
        return Err(format!(
            "protected session types have public trait seams: {:?}",
            audit.protected_trait_impls
        ));
    }
    if !audit.session_macros.is_empty() {
        return Err(format!(
            "session module contains unaudited item macros: {:?}",
            audit.session_macros
        ));
    }
    Ok(())
}

fn inspect_rust_items(items: &[syn::Item], audit: &mut SessionRustAst, in_session: bool) {
    for item in items {
        match item {
            syn::Item::Mod(module) => {
                if let Some((_, items)) = &module.content {
                    inspect_rust_items(items, audit, in_session || module.ident == "session");
                }
            }
            syn::Item::Impl(implementation) => {
                let Some(owner) = rust_type_last_ident(&implementation.self_ty) else {
                    continue;
                };
                let owner = owner.to_string();
                if implementation.trait_.is_some()
                    && matches!(
                        owner.as_str(),
                        "SessionAttemptPlan" | "SessionAttemptCompletion"
                    )
                {
                    audit.protected_trait_impls.push(owner);
                    continue;
                }
                if implementation.trait_.is_some() {
                    continue;
                }
                if owner == "SessionAttemptPlan" {
                    audit.plan_impls += 1;
                }
                if owner == "SessionAttemptCompletion" {
                    audit.completion_impls += 1;
                }
                for member in &implementation.items {
                    let syn::ImplItem::Fn(method) = member else {
                        continue;
                    };
                    if matches!(method.vis, syn::Visibility::Public(_)) {
                        if owner == "SessionAttemptPlan" {
                            audit.plan_methods.insert(method.sig.ident.to_string());
                        }
                        if owner == "SessionAttemptCompletion" {
                            audit
                                .completion_methods
                                .insert(method.sig.ident.to_string());
                        }
                        if rust_return_contains(&method.sig.output, "SessionAttemptCompletion") {
                            audit
                                .completion_producers
                                .insert(format!("{owner}::{}", method.sig.ident));
                        }
                    }
                }
            }
            syn::Item::Fn(function) if matches!(function.vis, syn::Visibility::Public(_)) => {
                if rust_return_contains(&function.sig.output, "SessionAttemptCompletion") {
                    audit
                        .completion_producers
                        .insert(format!("fn::{}", function.sig.ident));
                }
            }
            syn::Item::Type(alias) => {
                if rust_type_contains(&alias.ty, "SessionAttemptPlan")
                    || rust_type_contains(&alias.ty, "SessionAttemptCompletion")
                {
                    audit.protected_aliases.push(alias.ident.to_string());
                }
            }
            syn::Item::Use(import) => {
                inspect_use_aliases(&import.tree, audit);
            }
            syn::Item::Macro(item_macro) if in_session => {
                audit.session_macros.push(
                    item_macro
                        .mac
                        .path
                        .segments
                        .last()
                        .map(|segment| segment.ident.to_string())
                        .unwrap_or_else(|| "<unknown>".to_owned()),
                );
            }
            _ => {}
        }
    }
}

fn inspect_use_aliases(tree: &syn::UseTree, audit: &mut SessionRustAst) {
    match tree {
        syn::UseTree::Rename(rename)
            if rename.ident == "SessionAttemptPlan"
                || rename.ident == "SessionAttemptCompletion" =>
        {
            audit.protected_aliases.push(rename.rename.to_string());
        }
        syn::UseTree::Path(path) => inspect_use_aliases(&path.tree, audit),
        syn::UseTree::Group(group) => {
            for item in &group.items {
                inspect_use_aliases(item, audit);
            }
        }
        _ => {}
    }
}

fn rust_return_contains(output: &syn::ReturnType, expected: &str) -> bool {
    match output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => rust_type_contains(ty, expected),
    }
}

fn rust_type_contains(ty: &syn::Type, expected: &str) -> bool {
    match ty {
        syn::Type::Path(path) => path.path.segments.iter().any(|segment| {
            segment.ident == expected
                || match &segment.arguments {
                    syn::PathArguments::None => false,
                    syn::PathArguments::AngleBracketed(arguments) => {
                        arguments.args.iter().any(|argument| match argument {
                            syn::GenericArgument::Type(ty) => rust_type_contains(ty, expected),
                            syn::GenericArgument::AssocType(association) => {
                                rust_type_contains(&association.ty, expected)
                            }
                            _ => false,
                        })
                    }
                    syn::PathArguments::Parenthesized(arguments) => {
                        arguments
                            .inputs
                            .iter()
                            .any(|ty| rust_type_contains(ty, expected))
                            || rust_return_contains(&arguments.output, expected)
                    }
                }
        }),
        syn::Type::Array(array) => rust_type_contains(&array.elem, expected),
        syn::Type::BareFn(function) => rust_return_contains(&function.output, expected),
        syn::Type::Group(group) => rust_type_contains(&group.elem, expected),
        syn::Type::Paren(parenthesized) => rust_type_contains(&parenthesized.elem, expected),
        syn::Type::Ptr(pointer) => rust_type_contains(&pointer.elem, expected),
        syn::Type::Reference(reference) => rust_type_contains(&reference.elem, expected),
        syn::Type::Slice(slice) => rust_type_contains(&slice.elem, expected),
        syn::Type::Tuple(tuple) => tuple
            .elems
            .iter()
            .any(|ty| rust_type_contains(ty, expected)),
        _ => false,
    }
}

fn rust_type_last_ident(ty: &syn::Type) -> Option<&syn::Ident> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    path.path.segments.last().map(|segment| &segment.ident)
}

fn validate_creation_serializer_coverage(serializer: &str) -> CheckResult<()> {
    for field in [
        "project_files",
        "aggregate_project_bytes",
        "largest_project_file_bytes",
        "packages",
        "package_files",
        "largest_package_file_bytes",
        "package_tree_bytes",
        "font_containers",
        "font_candidates",
        "font_faces",
        "font_bytes",
        "discovery_variants",
        "discovery_restarts",
        "aggregate_file_bindings",
        "aggregate_logical_bytes",
        "override_count",
        "largest_override_bytes",
        "aggregate_override_bytes",
        "peak_stable_spool_bytes",
        "peak_retained_memory_bytes",
    ] {
        if !serializer.contains(&format!("reached.{field}")) {
            return Err(format!(
                "serializer probe omits reached Creation Resource field `{field}`"
            ));
        }
    }
    Ok(())
}

fn validate_session_admission_vectors() -> CheckResult<()> {
    #[derive(Clone, Copy)]
    struct Plan<'a> {
        token: &'a str,
        prepared: &'a str,
        permit: &'a str,
        limits: &'a str,
    }

    fn bind<'a>(
        plan: Plan<'a>,
        admitted_prepared: &'a str,
        admitted_limits: &'a str,
    ) -> Option<Plan<'a>> {
        (plan.prepared == admitted_prepared && plan.limits == admitted_limits).then_some(plan)
    }

    let plan = Plan {
        token: "attempt-a1",
        prepared: "compilation-c1",
        permit: "permit-a1",
        limits: "limits-l1",
    };
    let admitted = bind(plan, "compilation-c1", "limits-l1")
        .ok_or("coherent session attempt admission was refused")?;
    if admitted.token != "attempt-a1" || admitted.permit != "permit-a1" {
        return Err("session attempt admission lost its token or supersession permit".to_owned());
    }
    if bind(plan, "compilation-c2", "limits-l1").is_some()
        || bind(plan, "compilation-c1", "limits-l2").is_some()
    {
        return Err("session attempt admission accepted mixed plan facts".to_owned());
    }

    let mut active = Some("attempt-a1");
    let pending = Some("attempt-a2");
    let refused = "attempt-a1";
    let mut report_count = 0_u64;
    let mut publication_candidate = false;
    if active == Some(refused) {
        active = pending;
    }
    if active != Some("attempt-a2") || report_count != 0 || publication_candidate {
        return Err(
            "exact-token admission refusal fabricated or stranded session state".to_owned(),
        );
    }
    let stale_refusal = "attempt-old";
    if active == Some(stale_refusal) {
        active = None;
        report_count += 1;
        publication_candidate = true;
    }
    if active != Some("attempt-a2") || report_count != 0 || publication_candidate {
        return Err("stale admission refusal mutated the active session attempt".to_owned());
    }
    Ok(())
}

fn validate_capability_constants(
    schema: &Value,
    cases: &Value,
    summary: &mut Summary,
) -> CheckResult<()> {
    let constants = array_at(cases, "/capability_constants")?;
    for entry in constants {
        let definition = string_field(entry, "definition")?;
        let role = string_field(entry, "role")?;
        let actual = strings_at(schema, &format!("/$defs/{definition}/enum"))?;
        let expected = [
            format!("org.typst-pack/native-cli/{role}/1"),
            format!("org.typst-pack/dagger/{role}/1"),
        ];
        if actual != expected.iter().map(String::as_str).collect::<Vec<_>>() {
            return Err(format!(
                "capability definition `{definition}` differs: expected {expected:?}, got {actual:?}"
            ));
        }
        if actual.iter().any(|value| value.contains("/dagger-ci/")) {
            return Err(format!(
                "capability definition `{definition}` uses forbidden dagger-ci namespace"
            ));
        }
    }
    summary.capability_constants = constants.len();
    Ok(())
}

fn profile_u64(profile: &Value, pointer: &str, label: &str) -> CheckResult<u64> {
    profile
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label}: missing {pointer}"))?
        .parse::<u64>()
        .map_err(|error| format!("{label}: invalid u64 at {pointer}: {error}"))
}

fn validate_profile_coherence(profile: &Value, label: &str) -> CheckResult<()> {
    let archive_output = profile_u64(profile, "/representation/pack_archive/output_bytes", label)?;
    let archive_ingress = profile_u64(profile, "/pack_ingress/archive_bytes", label)?;
    let closure_output = profile_u64(
        profile,
        "/representation/closure_export/payload_bytes",
        label,
    )?;
    let closure_ingress =
        profile_u64(profile, "/pack_ingress/closure_export_payload_bytes", label)?;
    let entries = profile_u64(profile, "/pack_ingress/representation_entries", label)?;
    let transport_objects = profile_u64(profile, "/transport/objects", label)?;
    let transport_bytes = profile_u64(profile, "/transport/aggregate_bytes", label)?;
    let largest_transport = profile_u64(profile, "/transport/largest_object_bytes", label)?;
    let materialization_files = profile_u64(
        profile,
        "/representation/project_materialization/files",
        label,
    )?;
    let materialization_bytes = profile_u64(
        profile,
        "/representation/project_materialization/output_bytes",
        label,
    )?;
    let largest_blob = profile_u64(profile, "/pack_ingress/largest_physical_blob_bytes", label)?;
    let control = profile_u64(profile, "/pack_ingress/control_record_bytes", label)?;

    for field in [
        "override_count",
        "largest_override_bytes",
        "aggregate_override_bytes",
    ] {
        if profile.pointer(&format!("/compilation_preparation/limits/{field}"))
            != profile.pointer(&format!("/compilation/{field}"))
        {
            return Err(format!(
                "{label}: preparation limit {field} is not the explicit profile projection"
            ));
        }
    }
    if profile.pointer("/compilation_preparation/limits/diagnostic_entries")
        != profile.pointer("/canonical_diagnostic_policy/max_entries")
        || profile.pointer("/compilation_preparation/limits/diagnostic_entry_bytes")
            != profile.pointer("/canonical_diagnostic_policy/max_canonical_entry_bytes")
    {
        return Err(format!(
            "{label}: preparation diagnostic limits do not match the canonical policy ceilings"
        ));
    }

    if archive_output > archive_ingress
        || closure_output > closure_ingress
        || entries > transport_objects
        || materialization_files > transport_objects
        || closure_output > transport_bytes
        || materialization_bytes > transport_bytes
        || archive_output > largest_transport
        || largest_blob.max(control) > largest_transport
    {
        return Err(format!(
            "{label}: output-to-ingress or publication ceilings are incoherent"
        ));
    }
    Ok(())
}

fn validate_role_limit_projection(profile: &Value, label: &str) -> CheckResult<()> {
    let archive = format_controls(profile, "pack_archive_encoding");
    let closure = format_controls(profile, "closure_export");
    for field in [
        "logical_file_bindings",
        "logical_decoded_bytes",
        "control_record_bytes",
        "physical_blob_bytes",
        "largest_physical_blob_bytes",
        "representation_entries",
    ] {
        if archive.pointer(&format!("/limits/values/{field}"))
            != profile.pointer(&format!("/pack_ingress/{field}"))
            || closure.pointer(&format!("/limits/values/{field}"))
                != profile.pointer(&format!("/pack_ingress/{field}"))
        {
            return Err(format!(
                "{label}: role limit projection lost ingress field {field}"
            ));
        }
    }
    Ok(())
}

fn validate_resource_accounting_vectors() -> CheckResult<()> {
    // Equal logical bindings charge repeatedly; physical objects deduplicate by typed identity.
    let logical_bindings = [
        ("project:a", 100_u64),
        ("project:b", 100),
        ("package:x:a", 100),
    ];
    let logical_count = logical_bindings.len() as u64;
    let logical_bytes = logical_bindings
        .iter()
        .map(|(_, bytes)| *bytes)
        .sum::<u64>();
    let physical = [("typst-pack:exact-content:1:sha256:a", 100_u64)];
    if (logical_count, logical_bytes, physical.len() as u64) != (3, 300, 1) {
        return Err("logical repetition versus physical deduplication vector failed".to_owned());
    }

    // Mixed project/package/font totals exercise limit - 1, limit, and limit + 1.
    let file_limit = 100_u64;
    for (project, package, accepted) in [(49, 50, true), (50, 50, true), (50, 51, false)] {
        if (project + package <= file_limit) != accepted {
            return Err("mixed F_create boundary vector failed".to_owned());
        }
    }
    let byte_limit = 1_000_u64;
    for (project, package, font, accepted) in [
        (300, 399, 300, true),
        (300, 400, 300, true),
        (300, 400, 301, false),
    ] {
        if (project + package + font <= byte_limit) != accepted {
            return Err("mixed L_create boundary vector failed".to_owned());
        }
    }

    // Exact Package Specification remains part of logical identity; external dispositions
    // still charge logically while contributing no physical blob.
    let package_bindings = [
        ("@preview/a:1.0.0", "lib.typ", 7_u64),
        ("@preview/b:1.0.0", "lib.typ", 7_u64),
    ];
    let external_font_logical_bytes = 11_u64;
    let physical_embedded_bytes = 7_u64;
    if package_bindings.len() != 2
        || package_bindings
            .iter()
            .map(|(_, _, size)| size)
            .sum::<u64>()
            + external_font_logical_bytes
            != 25
        || physical_embedded_bytes != 7
    {
        return Err("package-specification or external-disposition accounting failed".to_owned());
    }

    // Zero-byte files charge counts; discovery overrides stay on separate dimensions;
    // repeated variants, restarts, and replay do not recharge one distinct binding.
    let observed_keys = ["project:empty", "project:empty", "project:empty"];
    let distinct = observed_keys.into_iter().collect::<BTreeSet<_>>();
    let override_bytes = 13_u64;
    if distinct.len() != 1 || override_bytes == 0 {
        return Err("zero-byte/replay/override accounting vector failed".to_owned());
    }

    // Unknown-length streams debit each observed chunk and ignore a lying zero size hint.
    let chunks = [3_u64, 5, 7];
    let lying_size_hint = 0_u64;
    let observed = chunks
        .iter()
        .try_fold(0_u64, |sum, chunk| sum.checked_add(*chunk));
    if lying_size_hint != 0 || observed != Some(15) {
        return Err("unknown-length incremental accounting failed".to_owned());
    }

    // Understood extensions contribute closed logical and physical projections.
    let extension_logical = (2_u64, 17_u64);
    let extension_physical = (1_u64, 9_u64);
    if extension_logical != (2, 17) || extension_physical != (1, 9) {
        return Err("semantic-extension accounting projection failed".to_owned());
    }

    let materialized_same_bytes = [("a.typ", 9_u64), ("b.typ", 9_u64)];
    if materialized_same_bytes.len() != 2
        || materialized_same_bytes
            .iter()
            .map(|(_, bytes)| bytes)
            .sum::<u64>()
            != 18
    {
        return Err(
            "Project Materialization per-path accounting deduplicated equal bytes".to_owned(),
        );
    }

    // Occupancy is peak-live, not cumulative; equal separate allocations charge separately.
    let mut live = 0_u64;
    let mut peak = 0_u64;
    for reservation in [64_u64, 64] {
        live = live.checked_add(reservation).ok_or("occupancy overflow")?;
        peak = peak.max(live);
    }
    live -= 64;
    if peak != 128 || live != 64 {
        return Err("peak occupancy vector failed".to_owned());
    }
    let mut shared = ReplayOccupancyLedger::default();
    let original = shared.reserve("shared-output", 100, 100)?;
    let second_owner = shared.reserve("shared-output", 100, 100)?;
    if (shared.live_spool, shared.live_memory) != (100, 100) {
        return Err("shared backing was charged more than once".to_owned());
    }
    shared.release(original)?;
    let stable_owner = shared.transfer(second_owner);
    if (shared.live_spool, shared.live_memory) != (100, 100) {
        return Err("ownership transfer released shared backing early".to_owned());
    }
    shared.release(stable_owner)?;
    if (shared.live_spool, shared.live_memory, shared.peak_spool) != (0, 0, 100) {
        return Err("ownership transfer cleanup or peak accounting failed".to_owned());
    }

    let mut native = ReplayOccupancyLedger::default();
    let native_reservation = native.reserve("native-spool", 100, 8)?;
    if (native.live_spool, native.live_memory) != (100, 8) {
        return Err("native spool resident accounting failed".to_owned());
    }
    native.release(native_reservation)?;

    let mut copied = ReplayOccupancyLedger::default();
    let output = copied.reserve("copied-output", 64, 64)?;
    let scratch = copied.reserve("scratch", 0, 32)?;
    let parser = copied.reserve("parsed-plan", 0, 16)?;
    if copied.peak_memory != 112 {
        return Err("copied output and scratch occupancy vector failed".to_owned());
    }
    copied.release(parser)?;
    copied.release(scratch)?;
    copied.release(output)?;

    for cleanup_path in [
        "success",
        "acquisition_failure",
        "decode_failure",
        "encode_failure",
        "cancelled",
        "deadline",
        "primary_failure",
    ] {
        let mut ledger = ReplayOccupancyLedger::default();
        let partial = ledger.reserve("partial", 64, 8)?;
        let scratch = ledger.reserve("failure-scratch", 0, 16)?;
        ledger.release(scratch)?;
        ledger.release(partial)?;
        if (ledger.live_spool, ledger.live_memory) != (0, 0)
            || ledger.peak_spool != 64
            || cleanup_path.is_empty()
        {
            return Err("occupancy cleanup-path vector failed".to_owned());
        }
    }
    live = 0;
    if live != 0 || peak != 128 {
        return Err("occupancy cleanup must release live bytes without lowering peak".to_owned());
    }
    Ok(())
}

fn epoch2_all_stored_archive_bytes(
    control_record_bytes: u64,
    blob_lengths: &[u64],
) -> CheckResult<u64> {
    const SENTINEL: u64 = 0xffff_ffff;
    let mut local_records = 0_u64;
    let mut central_directory = 0_u64;
    for (index, payload) in std::iter::once(&control_record_bytes)
        .chain(blob_lengths.iter())
        .enumerate()
    {
        let name = if index == 0 { 20_u64 } else { 88_u64 };
        let offset = local_records;
        let size_zip64 = *payload >= SENTINEL;
        let offset_zip64 = offset >= SENTINEL;
        let local_extra = if size_zip64 { 20_u64 } else { 0 };
        let central_extra = match (size_zip64, offset_zip64) {
            (false, false) => 0,
            (false, true) => 12,
            (true, false) => 20,
            (true, true) => 28,
        };
        local_records = local_records
            .checked_add(30 + name + local_extra)
            .and_then(|value| value.checked_add(*payload))
            .ok_or("local ZIP plan overflow")?;
        central_directory = central_directory
            .checked_add(46 + name + central_extra)
            .ok_or("central ZIP plan overflow")?;
    }
    let entries = 1_u64
        .checked_add(blob_lengths.len() as u64)
        .ok_or("entry count overflow")?;
    let zip64 = if zip64_trailer_required(entries, local_records, central_directory) {
        76
    } else {
        0
    };
    local_records
        .checked_add(central_directory)
        .and_then(|value| value.checked_add(22 + zip64))
        .ok_or_else(|| "archive ZIP plan overflow".to_owned())
}

fn zip64_trailer_required(entries: u64, local_records: u64, central_directory: u64) -> bool {
    entries >= 65_535 || local_records >= 0xffff_ffff || central_directory >= 0xffff_ffff
}

fn validate_all_stored_vectors() -> CheckResult<()> {
    for (entries, expected_overhead) in [
        (1_usize, 138_u64),
        (65_534, 16_514_454),
        (65_535, 16_514_782),
        (65_536, 16_515_034),
        (200_000, 50_399_962),
    ] {
        let blobs = vec![0_u64; entries - 1];
        let actual = epoch2_all_stored_archive_bytes(0, &blobs)?;
        if actual != expected_overhead {
            return Err(format!(
                "all-Stored N={entries}: expected {expected_overhead}, got {actual}"
            ));
        }
    }
    for (control, blobs, expected) in [
        (
            67_108_864_u64,
            vec![1_006_632_317_u64, 0],
            1_073_741_823_u64,
        ),
        (67_108_864, vec![1_006_632_318, 0], 1_073_741_824),
        (67_108_864, vec![1_006_632_319, 0], 1_073_741_825),
        (67_108_864, vec![2_080_373_637, 0, 0, 0], 2_147_483_647),
        (67_108_864, vec![2_080_373_638, 0, 0, 0], 2_147_483_648),
        (67_108_864, vec![2_080_373_639, 0, 0, 0], 2_147_483_649),
    ] {
        let actual = epoch2_all_stored_archive_bytes(control, &blobs)?;
        if actual != expected {
            return Err(format!(
                "archive boundary: expected {expected}, got {actual}"
            ));
        }
    }
    let below = epoch2_all_stored_archive_bytes(0, &[0xffff_fffe])?;
    let at = epoch2_all_stored_archive_bytes(0, &[0xffff_ffff])?;
    let above = epoch2_all_stored_archive_bytes(0, &[0x1_0000_0000])?;
    if (below, at, above) != (4_294_967_760, 4_294_967_801, 4_294_967_802) {
        return Err("ZIP64 size-sentinel vector failed".to_owned());
    }
    let offset_below = epoch2_all_stored_archive_bytes(4_294_967_244, &[0])?;
    let offset_at = epoch2_all_stored_archive_bytes(4_294_967_245, &[0])?;
    let size_and_offset = epoch2_all_stored_archive_bytes(4_294_967_245, &[0xffff_ffff])?;
    if (offset_below, offset_at, size_and_offset) != (4_294_967_710, 4_294_967_723, 8_589_935_054) {
        return Err("ZIP64 local-offset vector failed".to_owned());
    }
    for (entries, local, central, expected) in [
        (65_534, 0, 0, false),
        (65_535, 0, 0, true),
        (1, 0xffff_fffe, 0, false),
        (1, 0xffff_ffff, 0, true),
        (1, 0, 0xffff_fffe, false),
        (1, 0, 0xffff_ffff, true),
    ] {
        if zip64_trailer_required(entries, local, central) != expected {
            return Err("ZIP64 R/D/N trailer threshold vector failed".to_owned());
        }
    }
    if epoch2_all_stored_archive_bytes(u64::MAX, &[u64::MAX]).is_ok() {
        return Err("ZIP planner accepted checked-arithmetic overflow".to_owned());
    }
    for (control, physical, expected) in [
        (67_108_864_u64, 4_294_967_295_u64, 4_362_076_159_u64),
        (67_108_864, 4_294_967_296, 4_362_076_160),
        (67_108_864, 4_294_967_297, 4_362_076_161),
    ] {
        if control.checked_add(physical) != Some(expected) {
            return Err("Closure Export payload boundary vector failed".to_owned());
        }
    }
    Ok(())
}

fn compare_profile_ceilings(native: &Value, dagger: &Value, path: &str) -> CheckResult<()> {
    match (native, dagger) {
        (Value::Object(left), Value::Object(right)) => {
            for (key, left_value) in left {
                if let Some(right_value) = right.get(key) {
                    compare_profile_ceilings(left_value, right_value, &format!("{path}/{key}"))?;
                }
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            for (index, (left_value, right_value)) in left.iter().zip(right).enumerate() {
                compare_profile_ceilings(left_value, right_value, &format!("{path}/{index}"))?;
            }
        }
        (Value::String(left), Value::String(right)) if is_decimal(left) && is_decimal(right) => {
            if parse_decimal(left, path)? > parse_decimal(right, path)? {
                return Err(format!(
                    "profile relationship {path}: native `{left}` exceeds dagger `{right}`"
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn assert_canonical_numeric_strings(value: &Value, path: &str) -> CheckResult<()> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                assert_canonical_numeric_strings(child, &format!("{path}/{key}"))?;
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                assert_canonical_numeric_strings(child, &format!("{path}/{index}"))?;
            }
        }
        Value::String(text) if text.bytes().all(|byte| byte.is_ascii_digit()) => {
            if !is_decimal(text) {
                return Err(format!("{path}: noncanonical numeric string `{text}`"));
            }
        }
        Value::String(text)
            if text.starts_with('-')
                && text.len() > 1
                && text[1..].bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            if text == "-0" || text[1..].starts_with('0') {
                return Err(format!(
                    "{path}: noncanonical signed numeric string `{text}`"
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_decimal(value: &str) -> bool {
    value == "0"
        || (!value.is_empty()
            && !value.starts_with('0')
            && value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn parse_decimal(value: &str, context: &str) -> CheckResult<u128> {
    if !is_decimal(value) {
        return Err(format!("{context}: `{value}` is not a canonical decimal"));
    }
    value
        .parse::<u128>()
        .map_err(|error| format!("{context}: decimal overflow: {error}"))
}

fn validate_schema_cases(
    schema: &Value,
    native: &Value,
    dagger: &Value,
    cases: &Value,
    summary: &mut Summary,
) -> CheckResult<()> {
    for case in array_at(cases, "/schema_cases")? {
        let id = string_field(case, "id")?;
        let definition = string_field(case, "definition")?;
        let expected = bool_field(case, "valid")?;
        let instance = field(case, "instance")?;
        validate_definition(schema, definition, instance, expected, id)?;
        summary.schema_cases += 1;
    }

    for case in array_at(cases, "/generated_schema_cases")? {
        let id = string_field(case, "id")?;
        let definition = string_field(case, "definition")?;
        let builder = string_field(case, "builder")?;
        let expected = bool_field(case, "valid")?;
        let instance = build_generated_case(builder, native, dagger)?;
        validate_definition(schema, definition, &instance, expected, id)?;
        summary.generated_cases += 1;
    }

    validate_branch_contracts(schema, cases)?;
    Ok(())
}

fn validate_definition(
    root: &Value,
    definition: &str,
    instance: &Value,
    expected: bool,
    label: &str,
) -> CheckResult<()> {
    let mut schema = root.clone();
    let object = schema
        .as_object_mut()
        .ok_or_else(|| "root schema is not an object".to_owned())?;
    object.remove("anyOf");
    object.insert(
        "$ref".to_owned(),
        Value::String(format!("#/$defs/{definition}")),
    );
    let validator = jsonschema::draft202012::new(&schema)
        .map_err(|error| format!("{label}: compile $defs/{definition}: {error}"))?;
    let actual = validator.is_valid(instance);
    if actual != expected {
        let errors = validator
            .iter_errors(instance)
            .take(4)
            .map(|error| format!("{}: {error}", error.instance_path()))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!(
            "{label}: expected $defs/{definition} validity {expected}, got {actual}; {errors}"
        ));
    }
    Ok(())
}

fn validate_branch_contracts(schema: &Value, cases: &Value) -> CheckResult<()> {
    for contract in array_at(cases, "/branch_contracts")? {
        let definition = string_field(contract, "definition")?;
        let kind = string_field(contract, "kind")?;
        let branches = array_at(schema, &format!("/$defs/{definition}/oneOf"))?;
        let branch = branches
            .iter()
            .find(|branch| {
                branch
                    .pointer("/properties/kind/const")
                    .and_then(Value::as_str)
                    == Some(kind)
            })
            .ok_or_else(|| format!("$defs/{definition}: missing `{kind}` branch"))?;
        let actual = strings_at(branch, "/required")?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let expected = strings_field(contract, "required")?
            .into_iter()
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(format!(
                "$defs/{definition} `{kind}` required keys differ: expected {expected:?}, got {actual:?}"
            ));
        }
        let properties = object_at(branch, "/properties")?;
        for forbidden in strings_field(contract, "forbidden")? {
            if properties.contains_key(forbidden) {
                return Err(format!(
                    "$defs/{definition} `{kind}` unexpectedly exposes `{forbidden}`"
                ));
            }
        }
    }
    Ok(())
}

fn build_generated_case(builder: &str, native: &Value, dagger: &Value) -> CheckResult<Value> {
    let value = match builder {
        "create_request_rejected" => create_operation_request_rejected(native),
        "create_admission_refused" => create_operation_admission_refused(native),
        "create_report" => create_operation_report(native),
        "create_rejection_with_report" => {
            let mut value = create_operation_request_rejected(native);
            value
                .as_object_mut()
                .expect("generated object")
                .insert("report".to_owned(), creation_report(native));
            value
        }
        "representation_unsupported" => representation_unsupported(native),
        "representation_assertion_promoted" => {
            let mut value = representation_unsupported(native);
            *value
                .pointer_mut("/request/encoding_assertion")
                .expect("generated assertion") = json!("externally_asserted_and_byte_verified");
            value
        }
        "transport_refused" => transport_refused(native),
        "transport_refused_with_reached" => {
            let mut value = transport_refused(native);
            let object = value.as_object_mut().expect("generated object");
            object.insert("admission".to_owned(), transport_admission(native));
            object.insert("stage_ledger".to_owned(), transport_stage_ledger());
            value
        }
        "transport_admitted" => transport_admitted(native),
        "project_materialization" => project_materialization_receipt(native),
        "session_request_rejection" => session_request_rejection(native),
        "session_request_rejection_with_attempt" => {
            let mut value = session_request_rejection(native);
            value
                .as_object_mut()
                .expect("generated object")
                .insert("attempt".to_owned(), json!("forbidden-attempt"));
            value
        }
        "session_ingestion_failure" => session_ingestion_failure(native),
        "session_ingestion_failure_with_attempt" => {
            let mut value = session_ingestion_failure(native);
            value
                .as_object_mut()
                .expect("generated object")
                .insert("attempt".to_owned(), json!("forbidden-attempt"));
            value
        }
        "session_ingestion_old_broad_preparation" => {
            let mut value = session_ingestion_failure(native);
            value["ingestion_failure"]["policy"]["preparation"] = json!({
                "resource_profile": profile_reference(),
                "requested_limits": native["compilation"].clone(),
                "admitted_limits": native["compilation"].clone()
            });
            value
        }
        "session_preparation_profile_mismatch" => {
            let mut value = session_ingestion_failure(native);
            value["ingestion_failure"]["policy"]["preparation"]["limits"]["override_count"] =
                json!("100001");
            value
        }
        "session_running" => session_state("running", false),
        "session_retiring" => session_state("retiring", false),
        "session_retired_last_success" => session_state("retired", true),
        "create_report_dagger" => rewrite_for_dagger(create_operation_report(dagger)),
        "create_report_cli_dagger_classes" => {
            replace_capability_namespace(create_operation_report(native), "native-cli", "dagger")
        }
        "create_report_dagger_native_classes" => {
            let mut value = rewrite_for_dagger(create_operation_report(dagger));
            replace_capability_strings(&mut value, "/dagger/", "/native-cli/");
            value
        }
        "create_report_wrong_role_class" => {
            let mut value = create_operation_report(native);
            *value
                .pointer_mut("/report/operational_inventory/dependency_execution/packages/class")
                .expect("generated package class") =
                json!("org.typst-pack/native-cli/font-authority/1");
            value
        }
        "create_report_dagger_ci_class" => {
            replace_capability_namespace(create_operation_report(native), "native-cli", "dagger-ci")
        }
        "archive_encoding_report_with_spool" => archive_encoding_report(native, true),
        "archive_encoding_report_null_spool" => archive_encoding_report(native, false),
        "archive_encoding_report_missing_spool" => {
            let mut value = archive_encoding_report(native, true);
            value
                .as_object_mut()
                .expect("generated archive report")
                .remove("spool_receipt");
            value
        }
        "compile_report" => compile_operation_report(native),
        "compile_report_dagger" => rewrite_for_dagger(compile_operation_report(dagger)),
        "compile_refusal" => compilation_admission_refusal(native),
        "compile_refusal_cache" => {
            let mut value = compilation_admission_refusal(native);
            value["cache"] = semantic_cache_descriptor();
            value
        }
        "compilation_request" => compilation_operation_request(),
        "compilation_request_cache_enabled" => {
            let mut value = compilation_operation_request();
            value["cache"] = json!({"kind": "read_only", "availability": "required"});
            value
        }
        "compilation_inventory" => compilation_operational_inventory(native),
        "compilation_inventory_cache_hit" => {
            let mut value = compilation_operational_inventory(native);
            value["dependency_execution"]["cache_lookup"] = json!("verified_hit");
            value
        }
        "compilation_inventory_cache_descriptor" => {
            let mut value = compilation_operational_inventory(native);
            value["dependency_execution"]["cache_descriptor"] = semantic_cache_descriptor();
            value
        }
        "compilation_inventory_cache_isolation" => {
            let mut value = compilation_operational_inventory(native);
            value["dependency_execution"]["cache_isolation_domain_present"] = json!(true);
            value
        }
        "cache_provenance" => cache_provenance(),
        "cache_provenance_hit" => {
            json!({"provenance": "hit_verified", "evidence_scope": "historical"})
        }
        "watch_delivery" | "last_successful_delivery" => watch_delivery(native),
        "hostile_creation_refusal" => {
            let mut value = creation_admission_refusal(native);
            value["requested_trust"] = json!("hostile");
            value
        }
        "hostile_creation_report" => {
            let mut value = creation_report(native);
            value["operational_inventory"]["admission"]["requested_trust"] = json!("hostile");
            value
        }
        "hostile_compilation_refusal" => {
            let mut value = compilation_admission_refusal(native);
            value["requested_trust"] = json!("hostile");
            value
        }
        "hostile_compilation_report" => {
            let mut value = compilation_report(native, false);
            value["operational_inventory"]["admission"]["requested_trust"] = json!("hostile");
            value
        }
        "hostile_transport_receipt" => {
            let mut value = transport_refused(native);
            value["request"]["requested_trust"] = json!("hostile");
            value
        }
        "create_report_width_mismatch" => {
            let mut value = create_operation_report(native);
            value["report"]["operational_inventory"]["role_execution"]["engine_width"]["admitted"] =
                json!("2");
            value["report"]["operational_inventory"]["role_execution"]["domain"]["width"] =
                json!("2");
            value
        }
        "create_report_exact" => create_operation_report_exact(native),
        "create_report_exact_normalized_width_poison" => {
            exact_width_poison(create_operation_report_exact(native), "normalized")
        }
        "create_report_exact_core_requested_width_poison" => {
            exact_width_poison(create_operation_report_exact(native), "core_requested")
        }
        "create_report_exact_outer_admitted_width_poison" => {
            exact_width_poison(create_operation_report_exact(native), "outer_admitted")
        }
        "create_report_exact_report_admitted_width_poison" => {
            exact_width_poison(create_operation_report_exact(native), "report_admitted")
        }
        "create_report_exact_domain_width_poison" => {
            exact_width_poison(create_operation_report_exact(native), "domain")
        }
        "compile_report_width_mismatch" => {
            let mut value = compile_operation_report(native);
            value["report"]["operational_inventory"]["role_execution"]["engine_width"]["admitted"] =
                json!("2");
            value
        }
        "compile_report_exact" => compile_operation_report_exact(native),
        "compile_report_exact_normalized_width_poison" => {
            exact_width_poison(compile_operation_report_exact(native), "normalized")
        }
        "compile_report_exact_core_requested_width_poison" => {
            exact_width_poison(compile_operation_report_exact(native), "core_requested")
        }
        "compile_report_exact_outer_admitted_width_poison" => {
            exact_width_poison(compile_operation_report_exact(native), "outer_admitted")
        }
        "compile_report_exact_report_admitted_width_poison" => {
            exact_width_poison(compile_operation_report_exact(native), "report_admitted")
        }
        "compile_report_exact_domain_width_poison" => {
            exact_width_poison(compile_operation_report_exact(native), "domain")
        }
        "create_refusal_exact" => create_operation_admission_refused_exact(native),
        "create_report_missing_reached_packages" => {
            let mut value = create_operation_report(native);
            value["report"]["operational_inventory"]["resources"]["reached"]
                .as_object_mut()
                .expect("reached creation resources")
                .remove("packages");
            value
        }
        "representation_hostile" => {
            let mut value = representation_unsupported(native);
            value["admission"]["requested"]["trust"] = json!("hostile");
            value
        }
        "watch_delivery_session_mismatch" => {
            let mut value = watch_delivery(native);
            value["evaluation"]["session_instance"] = json!("session-other");
            value
        }
        "watch_delivery_result_mismatch" => {
            let mut value = watch_delivery(native);
            value["result_identity"] = json!(identity("compilation-result", 'e'));
            value
        }
        other => return Err(format!("unknown generated fixture builder `{other}`")),
    };
    Ok(value)
}

fn schema_header(name: &str) -> Value {
    json!({"name": name, "major": 1, "minor": 0})
}

fn producer() -> Value {
    producer_for("cli")
}

fn producer_for(adapter: &str) -> Value {
    json!({"adapter": adapter, "adapter_version": "prototype", "typst_pack_version": "0.4.0", "typst_version": "0.15.0"})
}

fn profile_reference() -> Value {
    profile_reference_for("native-cli")
}

fn profile_reference_for(name: &str) -> Value {
    json!({"name": name, "version": 1})
}

fn jobs(admitted: Option<&str>) -> Value {
    json!({"lexical": "omitted_automatic", "normalized": {"kind": "automatic"}, "admitted": admitted})
}

fn jobs_exact(width: &str, admitted: Option<&str>) -> Value {
    json!({"lexical": "exact_positive", "normalized": {"kind": "exact", "width": width}, "admitted": admitted})
}

fn font_scan_policy() -> Value {
    json!({"invalid_candidate": "warn_and_omit", "unreadable_candidate": "warn_and_omit"})
}

fn creation_request() -> Value {
    json!({
        "entrypoint": "main.typ",
        "variants": [{"ordinal": "0", "label": null, "target": "paged", "inputs": [], "features": [], "document_time": {"kind": "absent"}, "overrides": []}],
        "explicit_inclusions": [],
        "package_embedding": {"default": "embed", "rules": []},
        "font_embedding": {"default": "embed", "rules": []},
        "metadata": {"title": null, "description": null, "authors": [], "keywords": []},
        "annotations": []
    })
}

fn creation_request_rejection(native: &Value) -> Value {
    json!({
        "resource_profile": profile_reference(),
        "requested_limits": native["creation"].clone(),
        "admitted_limits": native["creation"].clone(),
        "issues": [{"code": "empty_collection", "role": "discovery_variant", "declaration_ordinal": null}]
    })
}

fn create_operation_request_rejected(native: &Value) -> Value {
    json!({
        "schema": schema_header("org.typst-pack.create-operation"),
        "producer": producer(),
        "kind": "request_rejected",
        "adapter_jobs": jobs(None),
        "font_scan_policy": font_scan_policy(),
        "request_rejection": creation_request_rejection(native)
    })
}

fn creation_operation_request() -> Value {
    json!({
        "network": "offline",
        "D": "1",
        "engine_width": {"kind": "automatic"},
        "K": null,
        "Q": null,
        "P": null,
        "font_scan_policy": font_scan_policy(),
        "required_capability_scopes": capability_scopes(false),
        "requested_execution_placement": "caller_thread",
        "requested_isolation": {"kind": "in_process", "claimed_enforcement": []},
        "interruption": "cooperative",
        "required_enforcement": [],
        "deadline": {"kind": "none"},
        "queue_timeout_ticks": null,
        "latency_target_ticks": null,
        "reporting": {"timing": false, "fine_engine_timing": false}
    })
}

fn evidence_descriptor() -> Value {
    json!({
        "role": "creation_evidence",
        "descriptor_version": 1,
        "class": "org.typst-pack/native-cli/creation-evidence/1",
        "capabilities": {"stability": "immutable", "race_closing_revalidation": false, "exact_key_revalidation": false, "opaque_scope_revalidation": false, "polling": false, "push_subscription": false, "cursor_replay": false, "network": "no_network"},
        "offered_scope": capability_scope("creation_evidence")
    })
}

fn authority_descriptor(role: &str) -> Value {
    let class_role = role.replace('_', "-");
    let mut descriptor = json!({
        "role": role,
        "descriptor_version": 1,
        "class": format!("org.typst-pack/native-cli/{class_role}/1"),
        "ordered_source_classes": ["caller_supplied"],
        "evidence": {"immutable_values": true, "exact_key_revalidation": false, "opaque_scope_revalidation": false, "polling": false, "push_subscription": false, "cursor_replay": false},
        "network": "no_network",
        "resolution_cache": "disabled",
        "private_caches": [],
        "offered_scope": capability_scope(role)
    });
    if role == "font_authority" {
        descriptor["supported_font_scan_policies"] = json!([font_scan_policy()]);
    }
    descriptor
}

fn capability_scopes(compilation: bool) -> Value {
    let mut scopes = vec![
        capability_scope("package_authority"),
        capability_scope("font_authority"),
    ];
    if compilation {
        scopes.push(capability_scope("reporting"));
    } else {
        scopes.push(capability_scope("creation_evidence"));
    }
    Value::Array(scopes)
}

fn capability_scope(role: &str) -> Value {
    match role {
        "package_authority" | "font_authority" => {
            json!({"role": role, "permitted_uses": ["resolution", "acquisition", "revalidation"], "coverage": "declared_dependency_requirements", "completeness": "complete"})
        }
        "creation_evidence" => {
            json!({"role": role, "permitted_uses": ["stabilization", "revalidation"], "coverage": "exact_operation_inputs", "completeness": "complete"})
        }
        "reporting" => {
            json!({"role": role, "permitted_uses": [], "coverage": "selected_report_channels", "completeness": "complete"})
        }
        "spool" => {
            json!({"role": role, "permitted_uses": ["stable_acquisition"], "coverage": "one_frozen_subject", "completeness": "complete"})
        }
        "pack_archive_acquisition" => {
            json!({"role": role, "permitted_uses": ["archive_acquisition"], "coverage": "one_frozen_subject", "completeness": "complete"})
        }
        "pack_archive_publication" => {
            json!({"role": role, "permitted_uses": ["archive_publication"], "coverage": "one_frozen_subject", "completeness": "complete"})
        }
        "project_materialization_publication" => {
            json!({"role": role, "permitted_uses": ["materialization_publication"], "coverage": "one_frozen_subject", "completeness": "complete"})
        }
        "closure_export_publication" => {
            json!({"role": role, "permitted_uses": ["closure_export_publication"], "coverage": "one_frozen_subject", "completeness": "complete"})
        }
        "compilation_delivery" => {
            json!({"role": role, "permitted_uses": ["compilation_delivery"], "coverage": "one_frozen_subject", "completeness": "complete"})
        }
        _ => unreachable!(),
    }
}

fn reporting_descriptor() -> Value {
    json!({
        "role": "reporting",
        "descriptor_version": 1,
        "class": "org.typst-pack/native-cli/reporting/1",
        "offered_scope": capability_scope("reporting")
    })
}

fn creation_admission_refusal(native: &Value) -> Value {
    json!({
        "stage": "admission",
        "operation_request": creation_operation_request(),
        "requested_trust": "partially_trusted",
        "resource_profile": profile_reference(),
        "requested_limits": native["creation"].clone(),
        "evidence": evidence_descriptor(),
        "packages": authority_descriptor("package_authority"),
        "fonts": authority_descriptor("font_authority"),
        "execution": null,
        "reporting": reporting_descriptor(),
        "reason": "capacity"
    })
}

fn create_operation_admission_refused(native: &Value) -> Value {
    json!({
        "schema": schema_header("org.typst-pack.create-operation"),
        "producer": producer(),
        "kind": "admission_refused",
        "adapter_jobs": jobs(None),
        "font_scan_policy": font_scan_policy(),
        "request": creation_request(),
        "admission_refusal": creation_admission_refusal(native)
    })
}

fn create_operation_admission_refused_exact(native: &Value) -> Value {
    let mut value = create_operation_admission_refused(native);
    value["adapter_jobs"] = jobs_exact("4", None);
    value["admission_refusal"]["operation_request"]["engine_width"] =
        json!({"kind": "exact", "width": "4"});
    value
}

fn creation_operational_inventory(native: &Value) -> Value {
    json!({
        "admission": {
            "requested_trust": "partially_trusted", "admitted_trust": "partially_trusted",
            "requested_network": "offline", "admitted_network": "offline",
            "contractual_no_network": true, "structural_network_enforcement": "not_claimed",
            "enforcement": {"requested": [], "admitted": []},
            "requested_capability_scopes": capability_scopes(false),
            "admitted_capability_scopes": capability_scopes(false),
            "requested_execution_placement": "caller_thread",
            "admitted_execution_placement": "caller_thread",
            "requested_isolation": {"kind": "in_process", "claimed_enforcement": []},
            "admitted_isolation": {"kind": "in_process", "claimed_enforcement": []}
        },
        "resources": {
            "profile": profile_reference(), "requested": native["creation"].clone(), "admitted": native["creation"].clone(),
            "reached": {"project_files": "1", "aggregate_project_bytes": "8", "largest_project_file_bytes": "8", "packages": "0", "package_files": "0", "largest_package_file_bytes": "0", "package_tree_bytes": "0", "font_containers": "0", "font_candidates": "0", "font_faces": "0", "font_bytes": "0", "discovery_variants": "1", "discovery_restarts": "0", "aggregate_file_bindings": "1", "aggregate_logical_bytes": "8", "override_count": "0", "largest_override_bytes": "0", "aggregate_override_bytes": "0", "peak_stable_spool_bytes": "0", "peak_retained_memory_bytes": "8"}
        },
        "dependency_execution": {
            "evidence": evidence_descriptor(), "packages": authority_descriptor("package_authority"), "fonts": authority_descriptor("font_authority"),
            "offline_roles_covered": ["creation_evidence", "package_authority", "font_authority"],
            "concurrency": {"symbol": "D", "requested": "1", "admitted": "1", "constraints": []},
            "font_scan_policy": {"requested": font_scan_policy(), "admitted": font_scan_policy(), "reached": {"kind": "not_reached"}},
            "reached_capability_scopes": []
        },
        "attempt_control": {
            "deadline": {"kind": "none"}, "cancellation_present": false, "monotonic_domain": "prototype-clock",
            "queue_timeout_ticks": null, "latency_target_ticks": null,
            "requested_interruption": "cooperative", "admitted_interruption": "cooperative", "winner": "terminal_commitment"
        },
        "role_execution": managed_caller_thread_execution("1"),
        "reporting": {
            "requested": {"timing": false, "fine_engine_timing": false},
            "admitted": {"timing": false, "fine_engine_timing": false},
            "timing": "not_requested", "fine_engine_timing": "not_requested", "fine_timing_lease_reached": false,
            "descriptor": reporting_descriptor()
        }
    })
}

fn managed_caller_thread_execution(width: &str) -> Value {
    json!({
        "kind": "caller_thread",
        "domain": {
            "kind": "managed", "identity": "org.typst-pack.engine-domain.prototype",
            "width": width, "fine_timing_lease_reached": false
        },
        "engine_width": {
            "symbol": "W", "kind": "exact", "requested": {"kind": "automatic"},
            "admitted": width, "constraints": ["verified_available_capacity"]
        },
        "reached_placement": {"kind": "reached", "placement": "caller_thread"},
        "reached_isolation": {"kind": "reached", "contract": {"kind": "in_process", "claimed_enforcement": []}}
    })
}

fn managed_caller_thread_execution_exact(width: &str) -> Value {
    json!({
        "kind": "caller_thread",
        "domain": {
            "kind": "managed", "identity": "org.typst-pack.engine-domain.prototype",
            "width": width, "fine_timing_lease_reached": false
        },
        "engine_width": {
            "symbol": "W", "kind": "exact", "requested": {"kind": "exact", "width": width},
            "admitted": width, "constraints": []
        },
        "reached_placement": {"kind": "reached", "placement": "caller_thread"},
        "reached_isolation": {"kind": "reached", "contract": {"kind": "in_process", "claimed_enforcement": []}}
    })
}

fn not_selected_caller_thread_execution() -> Value {
    json!({
        "kind": "caller_thread",
        "domain": {"kind": "not_selected"},
        "engine_width": {
            "symbol": "W", "kind": "exact", "requested": {"kind": "automatic"},
            "admitted": "1", "constraints": ["verified_available_capacity"]
        },
        "reached_placement": {"kind": "not_reached"},
        "reached_isolation": {"kind": "not_reached"}
    })
}

fn creation_report(native: &Value) -> Value {
    json!({
        "schema": schema_header("org.typst-pack.creation-report"),
        "producer": producer(),
        "terminal": "failed",
        "failure": {"phase": "admission", "cause": {"kind": "resource_limit"}},
        "phases": ["admission"],
        "diagnostics": [],
        "operational_inventory": creation_operational_inventory(native),
        "reporting": {"timing": "not_requested", "fine_engine_timing": "not_requested"}
    })
}

fn create_operation_report(native: &Value) -> Value {
    json!({
        "schema": schema_header("org.typst-pack.create-operation"),
        "producer": producer(),
        "kind": "creation_report",
        "adapter_jobs": jobs(Some("1")),
        "font_scan_policy": font_scan_policy(),
        "request": creation_request(),
        "report": creation_report(native),
        "archive_encoding": null,
        "publication": null,
        "post_commit": {"timings": {"status": "not_requested", "reason": null, "failure": null}}
    })
}

fn create_operation_report_exact(native: &Value) -> Value {
    let mut value = create_operation_report(native);
    value["adapter_jobs"] = jobs_exact("4", Some("4"));
    value["report"]["operational_inventory"]["role_execution"] =
        managed_caller_thread_execution_exact("4");
    value
}

fn format_controls(native: &Value, kind: &str) -> Value {
    let values = match kind {
        "pack_ingress" => native["pack_ingress"].clone(),
        "pack_archive_encoding" => json!({
            "logical_file_bindings": native["pack_ingress"]["logical_file_bindings"],
            "logical_decoded_bytes": native["pack_ingress"]["logical_decoded_bytes"],
            "control_record_bytes": native["pack_ingress"]["control_record_bytes"],
            "physical_blob_bytes": native["pack_ingress"]["physical_blob_bytes"],
            "largest_physical_blob_bytes": native["pack_ingress"]["largest_physical_blob_bytes"],
            "representation_entries": native["pack_ingress"]["representation_entries"],
            "maximum_expansion_ratio": native["pack_ingress"]["maximum_expansion_ratio"],
            "output_bytes": native["representation"]["pack_archive"]["output_bytes"],
            "stable_spool_bytes": native["representation"]["stable_spool_bytes"],
            "retained_memory_bytes": native["representation"]["retained_memory_bytes"]
        }),
        "closure_export" => json!({
            "logical_file_bindings": native["pack_ingress"]["logical_file_bindings"],
            "logical_decoded_bytes": native["pack_ingress"]["logical_decoded_bytes"],
            "control_record_bytes": native["pack_ingress"]["control_record_bytes"],
            "physical_blob_bytes": native["pack_ingress"]["physical_blob_bytes"],
            "largest_physical_blob_bytes": native["pack_ingress"]["largest_physical_blob_bytes"],
            "representation_entries": native["pack_ingress"]["representation_entries"],
            "payload_bytes": native["representation"]["closure_export"]["payload_bytes"],
            "stable_spool_bytes": native["representation"]["stable_spool_bytes"],
            "retained_memory_bytes": native["representation"]["retained_memory_bytes"]
        }),
        "project_materialization" => json!({
            "files": native["representation"]["project_materialization"]["files"],
            "output_bytes": native["representation"]["project_materialization"]["output_bytes"],
            "stable_spool_bytes": native["representation"]["stable_spool_bytes"],
            "retained_memory_bytes": native["representation"]["retained_memory_bytes"]
        }),
        "transport" => native["transport"].clone(),
        _ => unreachable!(),
    };
    json!({
        "trust": "partially_trusted", "network": "offline", "resource_profile": profile_reference(),
        "deadline": {"kind": "none"}, "cancellation_present": false, "interruption": "cooperative",
        "publication_strength": null, "cleanup_requirement": null,
        "limits": {"kind": kind, "values": values}, "enforcement": [],
        "timing_requested": false, "timing_reporting": false
    })
}

fn archive_read_request() -> Value {
    json!({
        "kind": "pack_archive_read",
        "input_archive_identity": identity("exact-content", 'a'),
        "expected_archive_identity": null,
        "verification": {"kind": "derive"},
        "asserted_archive_encoding_identity": first_party_archive_encoding_identity(),
        "encoding_assertion": "supplied_but_unevaluated"
    })
}

fn representation_unsupported(native: &Value) -> Value {
    json!({
        "schema": schema_header("org.typst-pack.format-receipt"),
        "producer": producer(),
        "contract_version": 1,
        "role": "pack_archive_read",
        "terminal": "admission_refused",
        "adapter_class": "cli",
        "request": archive_read_request(),
        "admission": {"kind": "refused", "stage": "admission", "requested": format_controls(native, "pack_ingress"), "reason": "unsupported_archive_encoding_recipe"}
    })
}

fn project_materialization_receipt(native: &Value) -> Value {
    let controls = format_controls(native, "project_materialization");
    json!({
        "schema": schema_header("org.typst-pack.format-receipt"),
        "producer": producer(),
        "contract_version": 1,
        "role": "project_materialization",
        "terminal": "success",
        "adapter_class": "cli",
        "request": {"kind": "project_materialization", "source_pack_identity": identity("pack", 'b')},
        "admission": {"kind": "admitted", "requested": controls.clone(), "admitted": controls},
        "effect": {
            "kind": "project_materialization",
            "common": {
                "stage": "complete",
                "accounting": {"kind": "project_materialization", "file_count": "1", "planned_output_bytes": "8", "produced_output_bytes": "8", "completed_output_bytes": "8", "occupancy": {"peak_stable_spool_bytes": "0", "peak_retained_memory_bytes": "8"}},
                "pack_exposed": true, "stable_value_completed": true, "timing": "not_requested",
                "publication": {"kind": "not_applicable"}, "cleanup_status": {"kind": "not_applicable"},
                "failure_class": "not_applicable", "failure_cause": null, "validation_rules": []
            },
            "file_count": "1", "aggregate_bytes": "8",
            "files": [{"path": "main.typ", "exact_bytes": "8", "content_identity": identity("exact-content", 'c')}]
        }
    })
}

fn archive_encoding_format_receipt(native: &Value) -> Value {
    let controls = format_controls(native, "pack_archive_encoding");
    json!({
        "schema": schema_header("org.typst-pack.format-receipt"),
        "producer": producer(),
        "contract_version": 1,
        "role": "pack_archive_encode",
        "terminal": "success",
        "adapter_class": "cli",
        "request": {
            "kind": "pack_archive_encode", "source_pack_identity": identity("pack", 'b'),
            "selected_archive_encoding_identity": first_party_archive_encoding_identity()
        },
        "admission": {"kind": "admitted", "requested": controls.clone(), "admitted": controls},
        "effect": {
            "kind": "pack_archive_encode",
            "common": {
                "stage": "complete",
                "accounting": {"kind": "pack_archive", "logical": {"file_bindings": "1", "decoded_bytes": "8"}, "physical": {"control_record_bytes": "2", "blob_count": "1", "blob_bytes": "8", "largest_blob_bytes": "8", "representation_entries": "2"}, "occupancy": {"peak_stable_spool_bytes": "400", "peak_retained_memory_bytes": "8"}, "input_bytes": null, "planned_output_bytes": "400", "produced_output_bytes": "400", "completed_output_bytes": "400"},
                "pack_exposed": false, "stable_value_completed": true, "timing": "not_requested",
                "publication": {"kind": "not_applicable"}, "cleanup_status": {"kind": "not_applicable"},
                "failure_class": "not_applicable", "failure_cause": null, "validation_rules": []
            },
            "control_record_identity": identity("exact-content", 'a'),
            "output_archive_identity": identity("exact-content", 'b'),
            "closure_export_tree_identity": identity("closure-export-tree-content", 'e')
        }
    })
}

fn archive_encoding_report(native: &Value, spool_attempted: bool) -> Value {
    json!({
        "receipt": archive_encoding_format_receipt(native),
        "spool_receipt": if spool_attempted { transport_admitted(native) } else { Value::Null }
    })
}

fn transport_descriptor() -> Value {
    json!({
        "role": "spool", "descriptor_version": 1,
        "class": "org.typst-pack/native-cli/spool-facility/1", "network": "no_network", "T": "1",
        "cleanup_requirement": ["complete_before_return"], "interruption": "cooperative",
        "enforcement": [], "timing_reporting": false,
        "offered_scope": capability_scope("spool")
    })
}

fn transport_request(native: &Value) -> Value {
    json!({
        "requested_trust": "partially_trusted", "resource_profile": profile_reference(),
        "requested_limits": {"kind": "spool", "values": native["spool"].clone()},
        "requested_network": "offline", "covered_roles": ["spool"], "contractual_no_network": true,
        "requested_structural_network_enforcement": "not_claimed", "T": "1", "requested_commit": null,
        "requested_cleanup_requirement": "complete_before_return", "interruption": "cooperative", "cancellation_present": false,
        "monotonic_domain": "prototype-clock", "required_enforcement": [], "timing_requested": false,
        "deadline": {"kind": "none"},
        "required_scope": capability_scope("spool")
    })
}

fn transport_refused(native: &Value) -> Value {
    json!({
        "schema": schema_header("org.typst-pack.transport-receipt"), "producer": producer(),
        "role": "spool", "status": "admission_refused", "stage": "admission", "adapter_class": "cli",
        "request": transport_request(native), "requested_subject": {"kind": "spool", "expected": null},
        "descriptor": transport_descriptor(), "reason": "capacity_unavailable"
    })
}

fn transport_admission(native: &Value) -> Value {
    json!({
        "requested_trust": "partially_trusted", "admitted_trust": "partially_trusted", "resource_profile": profile_reference(),
        "requested_limits": {"kind": "spool", "values": native["spool"].clone()},
        "admitted_limits": {"kind": "spool", "values": native["spool"].clone()},
        "requested_network": "offline", "admitted_network": "offline", "covered_roles": ["spool"], "contractual_no_network": true,
        "requested_structural_network_enforcement": "not_claimed", "admitted_structural_network_enforcement": "not_claimed",
        "requested_T": "1", "admitted_T": "1", "T_constraints": [], "requested_commit": null, "admitted_commit": null,
        "requested_cleanup_requirement": "complete_before_return", "admitted_cleanup_requirement": "complete_before_return",
        "requested_interruption": "cooperative", "admitted_interruption": "cooperative", "cancellation_present": false,
        "monotonic_domain": "prototype-clock", "enforcement": {"requested": [], "admitted": []},
        "timing_requested": false, "timing_reporting_admitted": false, "deadline": {"kind": "none"},
        "descriptor": transport_descriptor(),
        "requested_scope": capability_scope("spool"),
        "admitted_scope": capability_scope("spool")
    })
}

fn frozen_transport_object_count(subject: FrozenTransportSubject) -> String {
    public_transport_object_count(subject).to_string()
}

fn transport_stage_ledger() -> Value {
    let object_count = frozen_transport_object_count(FrozenTransportSubject::SingleValue);
    json!({
        "stages": ["admission", "plan_freeze", "spooling", "transfer", "complete"], "primary_terminal": "complete",
        "object_count": object_count, "transferred_bytes": "8", "actual_commit": null, "cleanup_outcome": "not_required",
        "residual_locator": null, "exposed_bytes": "8", "timing": {"status": "not_requested", "phases": []},
        "structural_network_enforcement_reached": "not_claimed", "enforcement_reached": [], "interruption_winner": "terminal_commitment",
        "reached_scope": capability_scope("spool")
    })
}

fn transport_admitted(native: &Value) -> Value {
    json!({
        "schema": schema_header("org.typst-pack.transport-receipt"), "producer": producer(),
        "role": "spool", "status": "transferred", "adapter_class": "cli",
        "admission": transport_admission(native), "stage_ledger": transport_stage_ledger(),
        "content_identity": null, "identities": [], "subject": {"kind": "spool", "expected": null, "actual": null}
    })
}

fn archive_publication_descriptor() -> Value {
    json!({
        "role": "pack_archive_publication",
        "descriptor_version": 1,
        "class": "org.typst-pack/native-cli/pack-archive-publisher/1",
        "network": "no_network",
        "T": "1",
        "commit_strengths": ["complete_collection_atomic"],
        "cleanup_requirement": ["complete_before_return"],
        "interruption": "cooperative",
        "enforcement": [],
        "timing_reporting": false,
        "offered_scope": capability_scope("pack_archive_publication")
    })
}

fn archive_publication_request(native: &Value) -> Value {
    json!({
        "requested_trust": "partially_trusted",
        "resource_profile": profile_reference(),
        "requested_limits": {"kind": "transfer", "values": native["transport"].clone()},
        "requested_network": "offline",
        "covered_roles": ["pack_archive_publication"],
        "contractual_no_network": true,
        "requested_structural_network_enforcement": "not_claimed",
        "T": "1",
        "requested_commit": "complete_collection_atomic",
        "requested_cleanup_requirement": "complete_before_return",
        "interruption": "cooperative",
        "cancellation_present": false,
        "monotonic_domain": "prototype-clock",
        "required_enforcement": [],
        "timing_requested": false,
        "deadline": {"kind": "none"},
        "required_scope": capability_scope("pack_archive_publication")
    })
}

fn archive_publication_refusal(native: &Value) -> Value {
    let source_pack = identity("pack", 'b');
    let source_archive = identity("exact-content", 'a');
    let source_tree = identity("closure-export-tree-content", 'e');
    let reason = "publication_commit_strength_unavailable";
    let mut controls = format_controls(native, "transport");
    controls["publication_strength"] = json!("complete_collection_atomic");
    controls["cleanup_requirement"] = json!("complete_before_return");
    json!({
        "format": {
            "schema": schema_header("org.typst-pack.format-receipt"),
            "producer": producer(),
            "contract_version": 1,
            "role": "pack_archive_publish",
            "terminal": "admission_refused",
            "adapter_class": "cli",
            "request": {
                "kind": "pack_archive_publish",
                "source_pack_identity": source_pack,
                "source_archive_identity": source_archive,
                "archive_encoding_identity": first_party_archive_encoding_identity(),
                "source_tree_identity": source_tree,
                "files": []
            },
            "admission": {"kind": "transport_refused", "stage": "admission", "requested": controls, "reason": reason}
        },
        "transport": {
            "terminal": {"status": "admission_refused"},
            "receipt": {
                "schema": schema_header("org.typst-pack.transport-receipt"),
                "producer": producer(),
                "role": "pack_archive_publication",
                "status": "admission_refused",
                "stage": "admission",
                "adapter_class": "cli",
                "request": archive_publication_request(native),
                "requested_subject": {"kind": "pack_archive_publication", "source_archive": identity("exact-content", 'a'), "archive_encoding": first_party_archive_encoding_identity()},
                "descriptor": archive_publication_descriptor(),
                "reason": reason
            }
        }
    })
}

fn closure_publication_refusal(native: &Value) -> Value {
    let mut value = archive_publication_refusal(native);
    value["format"]["role"] = json!("closure_export_publish");
    value["format"]["request"] = json!({
        "kind": "closure_export_publish",
        "source_pack_identity": identity("pack", 'b'),
        "source_tree_identity": identity("closure-export-tree-content", 'e'),
        "files": []
    });
    value["transport"]["receipt"]["role"] = json!("closure_export_publication");
    value["transport"]["receipt"]["request"]["covered_roles"] =
        json!(["closure_export_publication"]);
    value["transport"]["receipt"]["request"]["required_scope"] =
        capability_scope("closure_export_publication");
    value["transport"]["receipt"]["requested_subject"] = json!({
        "kind": "closure_export_publication",
        "pack": identity("pack", 'b'),
        "source_tree": identity("closure-export-tree-content", 'e')
    });
    value["transport"]["receipt"]["descriptor"]["role"] = json!("closure_export_publication");
    value["transport"]["receipt"]["descriptor"]["class"] =
        json!("org.typst-pack/native-cli/closure-export-publisher/1");
    value["transport"]["receipt"]["descriptor"]["offered_scope"] =
        capability_scope("closure_export_publication");
    value
}

fn publication_refusal_coherent(value: &Value) -> bool {
    value.pointer("/format/terminal").and_then(Value::as_str) == Some("admission_refused")
        && value
            .pointer("/format/admission/stage")
            .and_then(Value::as_str)
            == Some("admission")
        && value
            .pointer("/transport/receipt/stage")
            .and_then(Value::as_str)
            == Some("admission")
        && value.pointer("/format/admission/reason") == value.pointer("/transport/receipt/reason")
        && value.pointer("/format/request/source_archive_identity")
            == value.pointer("/transport/receipt/requested_subject/source_archive")
        && value.pointer("/format/request/archive_encoding_identity")
            == value.pointer("/transport/receipt/requested_subject/archive_encoding")
        && value.pointer("/transport/receipt/admission").is_none()
        && value.pointer("/transport/receipt/stage_ledger").is_none()
}

fn closure_publication_refusal_coherent(value: &Value) -> bool {
    value.pointer("/format/terminal").and_then(Value::as_str) == Some("admission_refused")
        && value
            .pointer("/format/admission/stage")
            .and_then(Value::as_str)
            == Some("admission")
        && value
            .pointer("/transport/receipt/stage")
            .and_then(Value::as_str)
            == Some("admission")
        && value.pointer("/format/admission/reason") == value.pointer("/transport/receipt/reason")
        && value.pointer("/format/request/source_pack_identity")
            == value.pointer("/transport/receipt/requested_subject/pack")
        && value.pointer("/format/request/source_tree_identity")
            == value.pointer("/transport/receipt/requested_subject/source_tree")
        && value.pointer("/transport/receipt/stage_ledger").is_none()
}

fn font_policy_coherent(inventory: &Value) -> bool {
    let requested = inventory.pointer("/dependency_execution/font_scan_policy/requested");
    let admitted = inventory.pointer("/dependency_execution/font_scan_policy/admitted");
    let reached = inventory.pointer("/dependency_execution/font_scan_policy/reached");
    requested.is_some()
        && requested == admitted
        && match reached
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str)
        {
            Some("not_reached") => true,
            Some("applied") => reached.and_then(|value| value.get("policy")) == admitted,
            _ => false,
        }
}

fn capability_scope_map(scopes: &Value) -> Option<BTreeMap<&str, (&str, &str, BTreeSet<&str>)>> {
    let mut result = BTreeMap::new();
    for scope in scopes.as_array()? {
        let role = scope.get("role")?.as_str()?;
        let coverage = scope.get("coverage")?.as_str()?;
        let completeness = scope.get("completeness")?.as_str()?;
        let uses = scope
            .get("permitted_uses")?
            .as_array()?
            .iter()
            .map(Value::as_str)
            .collect::<Option<BTreeSet<_>>>()?;
        if result
            .insert(role, (coverage, completeness, uses))
            .is_some()
        {
            return None;
        }
    }
    Some(result)
}

fn capability_scopes_are_subsets(requested: &Value, admitted: &Value, reached: &Value) -> bool {
    let Some(requested) = capability_scope_map(requested) else {
        return false;
    };
    let Some(admitted) = capability_scope_map(admitted) else {
        return false;
    };
    let Some(reached) = capability_scope_map(reached) else {
        return false;
    };
    requested
        .iter()
        .all(|(role, (coverage, completeness, uses))| {
            admitted.get(role).is_some_and(
                |(admitted_coverage, admitted_completeness, admitted_uses)| {
                    admitted_coverage == coverage
                        && admitted_completeness == completeness
                        && uses.is_subset(admitted_uses)
                },
            )
        })
        && reached
            .iter()
            .all(|(role, (coverage, completeness, uses))| {
                admitted.get(role).is_some_and(
                    |(admitted_coverage, admitted_completeness, admitted_uses)| {
                        admitted_coverage == coverage
                            && admitted_completeness == completeness
                            && uses.is_subset(admitted_uses)
                    },
                )
            })
}

fn compilation_operation_request() -> Value {
    json!({
        "network": "offline", "cache": {"kind": "disabled"}, "D": "1",
        "engine_width": {"kind": "automatic"}, "K": null, "Q": null, "P": null,
        "required_capability_scopes": capability_scopes(true),
        "requested_execution_placement": "caller_thread",
        "requested_isolation": {"kind": "in_process", "claimed_enforcement": []},
        "interruption": "cooperative", "deadline": {"kind": "none"},
        "queue_timeout_ticks": null, "latency_target_ticks": null, "required_enforcement": [],
        "reporting": {"diagnostic_projection": false, "diagnostic_source_bundle": false, "timing": false, "fine_engine_timing": false}
    })
}

fn compilation_admission_refusal(native: &Value) -> Value {
    json!({
        "stage": "admission", "compilation_identity": identity("compilation", 'c'),
        "operation_request": compilation_operation_request(), "requested_trust": "partially_trusted",
        "resource_profile": profile_reference(), "requested_limits": native["compilation"].clone(),
        "packages": authority_descriptor("package_authority"), "fonts": authority_descriptor("font_authority"),
        "cache": null, "execution": null, "reason": "capacity"
        ,"reporting": reporting_descriptor()
    })
}

fn inventory_leaf(value: Value, origin: &str) -> Value {
    json!({"value": value, "origin": origin, "status": "effective", "declaration_ordinal": null})
}

fn accepted_request_inventory() -> Value {
    json!([
        {"role": "pack", "identity": identity("pack", 'b'), "origin": "caller_supplied", "status": "effective"},
        {"role": "document_time", "value": {"kind": "absent"}, "origin": "core_defaulted", "status": "effective", "declaration_ordinal": null},
        {"role": "target", "value": "paged", "origin": "core_defaulted", "status": "effective", "declaration_ordinal": null},
        {"role": "output", "value": {"kind": "html", "format": inventory_leaf(json!("html"), "core_defaulted"), "pretty": inventory_leaf(json!(false), "core_defaulted"), "status": "effective"}},
        {
            "role": "diagnostics",
            "effective_policy": {"policy_version": 1, "max_entries": "20000", "max_canonical_entry_bytes": "67108864"},
            "version": inventory_leaf(json!(1), "core_defaulted"),
            "max_entries": inventory_leaf(json!("20000"), "core_defaulted"),
            "max_canonical_entry_bytes": inventory_leaf(json!("67108864"), "core_defaulted"),
            "status": "effective"
        },
        {"role": "engine", "identity": identity("engine", '1'), "origin": "core_derived", "status": "effective"},
        {"role": "exporter", "identity": identity("exporter", '2'), "origin": "core_derived", "status": "effective"}
    ])
}

fn compilation_operational_inventory(native: &Value) -> Value {
    json!({
        "admission": {
            "requested_trust": "partially_trusted", "admitted_trust": "partially_trusted",
            "requested_network": "offline", "admitted_network": "offline", "contractual_no_network": true,
            "structural_network_enforcement": "not_claimed", "enforcement": {"requested": [], "admitted": []},
            "requested_capability_scopes": capability_scopes(true),
            "admitted_capability_scopes": capability_scopes(true),
            "requested_execution_placement": "caller_thread",
            "admitted_execution_placement": "caller_thread",
            "requested_isolation": {"kind": "in_process", "claimed_enforcement": []},
            "admitted_isolation": {"kind": "in_process", "claimed_enforcement": []}
        },
        "resources": {"profile": profile_reference(), "requested": native["compilation"].clone(), "admitted": native["compilation"].clone()},
        "dependency_execution": {
            "packages": authority_descriptor("package_authority"), "fonts": authority_descriptor("font_authority"),
            "cache_descriptor": null, "cache_policy": {"kind": "disabled"}, "cache_lookup": "disabled",
            "cache_isolation_domain_present": false, "offline_roles_covered": ["package_authority", "font_authority"],
            "concurrency": {"symbol": "D", "requested": "1", "admitted": "1", "constraints": []},
            "reached_capability_scopes": []
        },
        "attempt_control": {
            "deadline": {"kind": "none"}, "cancellation_present": false, "monotonic_domain": "prototype-clock",
            "queue_timeout_ticks": null, "latency_target_ticks": null, "session_supersession": "not_applicable",
            "requested_interruption": "cooperative", "admitted_interruption": "cooperative", "winner": "terminal_commitment"
        },
        "role_execution": managed_caller_thread_execution("1"),
        "reporting": {
            "requested": {"diagnostic_projection": false, "diagnostic_source_bundle": false, "timing": false, "fine_engine_timing": false},
            "admitted": {"diagnostic_projection": false, "diagnostic_source_bundle": false, "timing": false, "fine_engine_timing": false},
            "diagnostic_projection": "not_requested", "diagnostic_sources": "not_requested", "timing": "not_requested",
            "fine_engine_timing": "not_requested", "fine_timing_lease_reached": false,
            "descriptor": reporting_descriptor()
        }
    })
}

fn report_channel_not_requested() -> Value {
    json!({"status": "not_requested", "data": null})
}

fn cache_provenance() -> Value {
    json!({"provenance": "disabled", "evidence_scope": "none"})
}

fn compilation_report(native: &Value, succeeded: bool) -> Value {
    let terminal = if succeeded {
        json!({"kind": "semantic_result", "status": "succeeded", "document": {"target": "paged", "source_page_count": "1"}})
    } else {
        json!({"kind": "operation_outcome", "phase": "admission", "cause": {"kind": "deadline"}})
    };
    let result_identity = if succeeded {
        json!(identity("compilation-result", 'd'))
    } else {
        Value::Null
    };
    let mut operational_inventory = compilation_operational_inventory(native);
    if !succeeded {
        operational_inventory["role_execution"] = not_selected_caller_thread_execution();
    }
    json!({
        "schema": schema_header("org.typst-pack.compilation-report"), "producer": producer(),
        "request_inventory": accepted_request_inventory(), "operational_inventory": operational_inventory,
        "cache_provenance": cache_provenance(),
        "report_projection": {
            "terminal": terminal, "compilation_identity": identity("compilation", 'c'), "result_identity": result_identity,
            "artifacts": [], "diagnostic_summary": {"retained_entries": "0", "retained_canonical_bytes": "0", "completion": null},
            "canonical_diagnostic_policy": {"policy_version": 1, "max_entries": "20000", "max_canonical_entry_bytes": "67108864"},
            "canonical_diagnostics": report_channel_not_requested(),
            "canonical_evidence": {"status": "not_requested", "value": null},
            "diagnostic_sources": report_channel_not_requested(), "request_values": report_channel_not_requested(),
            "override_bytes": report_channel_not_requested(), "backing_locators": report_channel_not_requested(),
            "adapter_detail": report_channel_not_requested()
        }
    })
}

fn compile_operation_report(native: &Value) -> Value {
    json!({
        "schema": schema_header("org.typst-pack.compile-operation"), "producer": producer(),
        "kind": "compilation_report", "adapter_jobs": jobs(Some("1")), "ingress": null,
        "adapter_input": {"status": "not_requested", "reason": null, "failure": null},
        "report": compilation_report(native, false), "delivery": null,
        "post_commit": {
            "dependencies": {"status": "not_requested", "reason": null, "failure": null},
            "timings": {"status": "not_requested", "reason": null, "failure": null},
            "viewer": {"status": "not_requested", "reason": null, "failure": null}
        }
    })
}

fn compile_operation_request_rejected(native: &Value) -> Value {
    json!({
        "schema": schema_header("org.typst-pack.compile-operation"),
        "producer": producer(),
        "kind": "request_rejected",
        "adapter_jobs": jobs_exact("999", None),
        "ingress": null,
        "adapter_input": {"status": "not_requested", "reason": null, "failure": null},
        "request_rejection": compilation_request_rejection(native)
    })
}

fn compile_operation_report_exact(native: &Value) -> Value {
    let mut value = compile_operation_report(native);
    value["report"] = compilation_report(native, true);
    value["adapter_jobs"] = jobs_exact("4", Some("4"));
    value["report"]["operational_inventory"]["role_execution"] =
        managed_caller_thread_execution_exact("4");
    value
}

fn exact_width_poison(mut operation: Value, target: &str) -> Value {
    match target {
        "normalized" => operation["adapter_jobs"]["normalized"]["width"] = json!("5"),
        "core_requested" => {
            operation["report"]["operational_inventory"]["role_execution"]["engine_width"]["requested"]
                ["width"] = json!("5")
        }
        "outer_admitted" => operation["adapter_jobs"]["admitted"] = json!("5"),
        "report_admitted" => {
            operation["report"]["operational_inventory"]["role_execution"]["engine_width"]["admitted"] =
                json!("5")
        }
        "domain" => {
            operation["report"]["operational_inventory"]["role_execution"]["domain"]["width"] =
                json!("5")
        }
        _ => unreachable!(),
    }
    operation
}

fn semantic_cache_descriptor() -> Value {
    json!({
        "role": "semantic_result_cache", "descriptor_version": 1,
        "class": "org.typst-pack/native-cli/semantic-result-cache/1",
        "isolation_domain_present": true, "network": "no_network"
    })
}

fn replace_capability_strings(value: &mut Value, from: &str, to: &str) {
    match value {
        Value::Object(object) => {
            for child in object.values_mut() {
                replace_capability_strings(child, from, to);
            }
        }
        Value::Array(values) => {
            for child in values {
                replace_capability_strings(child, from, to);
            }
        }
        Value::String(text) if text.starts_with("org.typst-pack/") && text.contains(from) => {
            *text = text.replace(from, to);
        }
        _ => {}
    }
}

fn rewrite_for_dagger(mut value: Value) -> Value {
    rewrite_producer_metadata(&mut value, "dagger", "dagger-ci");
    replace_capability_strings(&mut value, "/native-cli/", "/dagger/");
    value
}

fn replace_capability_namespace(mut value: Value, from: &str, to: &str) -> Value {
    replace_capability_strings(&mut value, &format!("/{from}/"), &format!("/{to}/"));
    value
}

fn rewrite_producer_metadata(value: &mut Value, adapter: &str, profile: &str) {
    match value {
        Value::Object(object) => {
            if object.contains_key("adapter")
                && object.contains_key("adapter_version")
                && object.contains_key("typst_pack_version")
            {
                object.insert("adapter".to_owned(), json!(adapter));
            }
            if object
                .get("adapter_class")
                .and_then(Value::as_str)
                .is_some()
            {
                object.insert("adapter_class".to_owned(), json!(adapter));
            }
            if object.get("name").and_then(Value::as_str) == Some("native-cli") {
                object.insert("name".to_owned(), json!(profile));
            }
            for child in object.values_mut() {
                rewrite_producer_metadata(child, adapter, profile);
            }
        }
        Value::Array(values) => {
            for child in values {
                rewrite_producer_metadata(child, adapter, profile);
            }
        }
        _ => {}
    }
}

fn delivery_transport_descriptor() -> Value {
    json!({
        "role": "compilation_delivery", "descriptor_version": 1,
        "class": "org.typst-pack/native-cli/compilation-delivery/1", "network": "no_network", "T": "1",
        "commit_strengths": ["complete_collection_atomic"],
        "cleanup_requirement": ["complete_before_return"], "interruption": "cooperative",
        "enforcement": [], "timing_reporting": false,
        "offered_scope": capability_scope("compilation_delivery")
    })
}

fn delivery_transport_admission(native: &Value) -> Value {
    json!({
        "requested_trust": "partially_trusted", "admitted_trust": "partially_trusted", "resource_profile": profile_reference(),
        "requested_limits": {"kind": "transfer", "values": native["transport"].clone()},
        "admitted_limits": {"kind": "transfer", "values": native["transport"].clone()},
        "requested_network": "offline", "admitted_network": "offline", "covered_roles": ["compilation_delivery"],
        "contractual_no_network": true, "requested_structural_network_enforcement": "not_claimed",
        "admitted_structural_network_enforcement": "not_claimed", "requested_T": "1", "admitted_T": "1",
        "T_constraints": [], "requested_commit": "complete_collection_atomic", "admitted_commit": "complete_collection_atomic",
        "requested_cleanup_requirement": "complete_before_return", "admitted_cleanup_requirement": "complete_before_return",
        "requested_interruption": "cooperative", "admitted_interruption": "cooperative", "cancellation_present": false,
        "monotonic_domain": "prototype-clock", "enforcement": {"requested": [], "admitted": []},
        "timing_requested": false, "timing_reporting_admitted": false, "deadline": {"kind": "none"},
        "descriptor": delivery_transport_descriptor(),
        "requested_scope": capability_scope("compilation_delivery"),
        "admitted_scope": capability_scope("compilation_delivery")
    })
}

fn committed_delivery_outcome(native: &Value) -> Value {
    let result = identity("compilation-result", 'd');
    let object_count =
        frozen_transport_object_count(FrozenTransportSubject::CompilationDelivery { artifacts: 0 });
    json!({
        "report": compilation_report(native, true),
        "transport": {
            "terminal": {"status": "succeeded"},
            "receipt": {
                "schema": schema_header("org.typst-pack.transport-receipt"), "producer": producer(),
                "role": "compilation_delivery", "status": "committed", "adapter_class": "cli",
                "admission": delivery_transport_admission(native),
                "stage_ledger": {
                    "stages": ["admission", "plan_freeze", "transfer", "commit", "complete"],
                    "primary_terminal": "complete", "object_count": object_count, "transferred_bytes": "0",
                    "actual_commit": "complete_collection_atomic", "cleanup_outcome": "not_required", "residual_locator": null,
                    "exposed_bytes": "0", "timing": {"status": "not_requested", "phases": []},
                    "structural_network_enforcement_reached": "not_claimed", "enforcement_reached": [],
                    "interruption_winner": "terminal_commitment",
                    "reached_scope": capability_scope("compilation_delivery")
                },
                "content_identity": null,
                "identities": [
                    {"kind": "compilation", "value": identity("compilation", 'c')},
                    {"kind": "result", "value": result.clone()}
                ],
                "subject": {"kind": "compilation_delivery", "compilation": identity("compilation", 'c'), "result": result, "artifacts": []}
            }
        }
    })
}

fn watch_delivery(native: &Value) -> Value {
    json!({
        "session_instance": "session-1", "revision": revision_identity(1), "evaluation": evaluation_identity(1, 2),
        "publication_sequence": publication_identity(3), "result_identity": identity("compilation-result", 'd'),
        "outcome": committed_delivery_outcome(native)
    })
}

fn compilation_request_rejection(native: &Value) -> Value {
    json!({
        "preparation_policy": native["compilation_preparation"]["policy"].clone(),
        "preparation_limits": native["compilation_preparation"]["limits"].clone(),
        "request_inventory": [],
        "issues": [{"code": "unsupported_feature", "role": "feature", "declaration_ordinal": "0", "referenced_inventory_ordinal": null}]
    })
}

fn session_policy(native: &Value) -> Value {
    json!({
        "mode": "latest_only_complete_coverage",
        "preparation": {
            "origin": "adapter_profile",
            "resource_profile": profile_reference(),
            "policy": native["compilation_preparation"]["policy"].clone(),
            "limits": native["compilation_preparation"]["limits"].clone()
        }
    })
}

fn session_request_rejection(native: &Value) -> Value {
    json!({
        "publication": publication_identity(1), "revision": revision_identity(1), "evaluation": evaluation_identity(1, 1),
        "kind": "request_rejected", "request_rejection": compilation_request_rejection(native),
        "currentness": {"kind": "current_as_of_poll", "observations": []}
    })
}

fn session_ingestion_failure(native: &Value) -> Value {
    json!({
        "publication": publication_identity(2), "revision": revision_identity(2), "evaluation": evaluation_identity(2, 1),
        "kind": "ingestion_failure",
        "ingestion_failure": {"safe_code": "source-unavailable", "failed_request_sources": ["request:project"], "policy": session_policy(native)},
        "currentness": {"kind": "unverified", "uncovered": {"request_sources": ["request:project"], "dependencies": []}}
    })
}

fn session_state(lifecycle: &str, last_success: bool) -> Value {
    let active = if lifecycle == "retiring" {
        json!({"attempt": attempt_identity(2, 1, 1), "revision": revision_identity(2), "evaluation": evaluation_identity(2, 1), "state": "draining"})
    } else {
        Value::Null
    };
    let last = if last_success {
        json!({
            "revision": revision_identity(1), "evaluation": evaluation_identity(1, 2), "publication": publication_identity(3),
            "result": identity("compilation-result", 'd'), "currentness": {"kind": "current_as_of_poll", "observations": []}
        })
    } else {
        Value::Null
    };
    json!({
        "session_instance": "session-1", "pack_identity": identity("pack", 'b'), "lifecycle": lifecycle,
        "latest_revision": if lifecycle == "running" { Value::Null } else { revision_identity(2) },
        "latest_evaluation": if lifecycle == "running" { Value::Null } else { evaluation_identity(2, 1) },
        "active_or_draining": active, "latest_pending": null, "publication": null, "last_successful": last
    })
}

fn revision_identity(ordinal: u64) -> Value {
    json!({"session_instance": "session-1", "ordinal": ordinal.to_string()})
}

fn evaluation_identity(revision: u64, ordinal: u64) -> Value {
    json!({"session_instance": "session-1", "revision": revision_identity(revision), "ordinal": ordinal.to_string()})
}

fn attempt_identity(revision: u64, evaluation: u64, ordinal: u64) -> Value {
    json!({"session_instance": "session-1", "evaluation": evaluation_identity(revision, evaluation), "ordinal": ordinal.to_string()})
}

fn publication_identity(sequence: u64) -> Value {
    json!({"session_instance": "session-1", "sequence": sequence.to_string()})
}

fn identity(role: &str, digit: char) -> String {
    format!(
        "typst-pack:{role}:1:sha256:{}",
        digit.to_string().repeat(64)
    )
}

fn first_party_archive_encoding_identity() -> &'static str {
    "typst-pack:archive-encoding:1:sha256:4e338d8a54d234ca28392ecf79386944757e0e4adf750192e21311d6b2491170"
}

fn validate_semantics(
    schema: &Value,
    cases: &Value,
    native: &Value,
    dagger: &Value,
    summary: &mut Summary,
) -> CheckResult<()> {
    let schema_cases = array_at(cases, "/schema_cases")?;
    let ordered = schema_cases
        .iter()
        .find(|case| string_field(case, "id") == Ok("transport-ledger-admitted"))
        .ok_or_else(|| "missing ordered transport ledger fixture".to_owned())?;
    validate_transport_ledger(
        field(ordered, "instance")?,
        true,
        "ordered transport ledger",
    )?;
    let poison = schema_cases
        .iter()
        .find(|case| string_field(case, "id") == Ok("transport-ledger-order-semantic-poison"))
        .ok_or_else(|| "missing transport ledger order poison".to_owned())?;
    validate_transport_ledger(
        field(poison, "instance")?,
        false,
        "transport ledger order poison",
    )?;

    for disposition in ["omit", "warn_and_omit", "reject_catalog"] {
        let policy = json!({"invalid_candidate": disposition, "unreadable_candidate": disposition});
        validate_definition(
            schema,
            "font_scan_policy",
            &policy,
            true,
            "font scan policy",
        )?;
        summary.semantic_cases += 1;
    }
    let creation_inventory = creation_operational_inventory(native);
    if !font_policy_coherent(&creation_inventory) {
        return Err(
            "Creation Report did not preserve requested/admitted Font Scan Policy".to_owned(),
        );
    }
    let mut applied_policy = creation_inventory.clone();
    applied_policy["dependency_execution"]["font_scan_policy"]["reached"] = json!({
        "kind": "applied",
        "policy": font_scan_policy(),
        "diagnostic_count": "0"
    });
    if !font_policy_coherent(&applied_policy) {
        return Err("applied Font Scan Policy did not reach the authority unchanged".to_owned());
    }
    let mut mismatched_policy = applied_policy;
    mismatched_policy["dependency_execution"]["font_scan_policy"]["reached"]["policy"] =
        json!({"invalid_candidate": "reject_catalog", "unreadable_candidate": "reject_catalog"});
    if font_policy_coherent(&mismatched_policy) {
        return Err("returned Font Scan Policy mismatch did not fail closed".to_owned());
    }
    let mut unsupported_policy = creation_admission_refusal(native);
    unsupported_policy["reason"] = json!("font_scan_policy_unavailable");
    if unsupported_policy["stage"] != "admission" || unsupported_policy.get("report").is_some() {
        return Err("unsupported Font Scan Policy did not refuse before scanning".to_owned());
    }
    summary.semantic_cases += 4;

    let not_selected = json!({"kind": "not_selected"});
    validate_definition(
        schema,
        "engine_runtime_domain_selection",
        &not_selected,
        true,
        "domain not selected",
    )?;
    let not_selected_with_width = json!({"kind": "not_selected", "width": "1"});
    validate_definition(
        schema,
        "engine_runtime_domain_selection",
        &not_selected_with_width,
        false,
        "domain not selected with inferred width",
    )?;
    let admission_phase_report = compilation_report(native, false);
    if admission_phase_report
        .pointer("/report_projection/terminal/phase")
        .and_then(Value::as_str)
        != Some("admission")
        || admission_phase_report
            .pointer("/operational_inventory/role_execution/domain/kind")
            .and_then(Value::as_str)
            != Some("not_selected")
        || admission_phase_report
            .pointer("/operational_inventory/role_execution/reached_placement/kind")
            .and_then(Value::as_str)
            != Some("not_reached")
    {
        return Err(
            "admission-phase interruption fabricated reached execution/domain facts".to_owned(),
        );
    }
    for predispatch_terminal in [
        "cache_hit",
        "dependency_failure",
        "queue_refusal",
        "cancellation_before_dispatch",
        "worker_setup_failure_before_assignment",
    ] {
        let domain = json!({"kind": "not_selected"});
        if domain["kind"] != "not_selected" || predispatch_terminal.is_empty() {
            return Err("pre-dispatch terminal selected an Engine Runtime Domain".to_owned());
        }
    }
    summary.semantic_cases += 2;

    let archive_object_count = frozen_transport_object_count(FrozenTransportSubject::SingleValue);
    let cleanup_primary = json!({
        "stages": ["admission", "plan_freeze", "transfer", "commit", "cleanup", "complete"],
        "primary_terminal": "cleanup", "object_count": archive_object_count, "transferred_bytes": "8",
        "actual_commit": "complete_collection_atomic", "cleanup_outcome": "cleanup_failed",
        "residual_locator": null, "exposed_bytes": "8",
        "timing": {"status": "not_requested", "phases": []},
        "structural_network_enforcement_reached": "not_claimed", "enforcement_reached": [],
        "interruption_winner": "terminal_commitment", "reached_scope": capability_scope("spool")
    });
    validate_definition(
        schema,
        "transport_stage_ledger",
        &cleanup_primary,
        true,
        "post-commit cleanup-primary ledger",
    )?;
    validate_transport_ledger(&cleanup_primary, true, "post-commit cleanup-primary ledger")?;
    let mut cleanup_receipt = transport_admitted(native);
    cleanup_receipt["status"] = json!("failed");
    cleanup_receipt["stage_ledger"] = cleanup_primary.clone();
    let cleanup_outcome = json!({
        "terminal": {"status": "failed", "primary": {"kind": "cleanup"}, "cleanup_outcome": "cleanup_failed"},
        "receipt": cleanup_receipt
    });
    validate_definition(
        schema,
        "transport_outcome",
        &cleanup_outcome,
        true,
        "cleanup-primary transport outcome",
    )?;
    if !transport_outcome_primary_coherent(&cleanup_outcome) {
        return Err("cleanup-primary ledger disagrees with outer failure".to_owned());
    }
    let closure_object_count =
        frozen_transport_object_count(FrozenTransportSubject::ClosureExport { entries: 200_000 });
    let transfer_primary = json!({
        "stages": ["admission", "plan_freeze", "transfer", "cleanup", "complete"],
        "primary_terminal": "transfer", "object_count": closure_object_count, "transferred_bytes": "8",
        "actual_commit": null, "cleanup_outcome": "cleanup_failed", "residual_locator": null,
        "exposed_bytes": "8", "timing": {"status": "not_requested", "phases": []},
        "structural_network_enforcement_reached": "not_claimed", "enforcement_reached": [],
        "interruption_winner": null, "reached_scope": capability_scope("closure_export_publication")
    });
    validate_definition(
        schema,
        "transport_stage_ledger",
        &transfer_primary,
        true,
        "partial transfer ledger",
    )?;
    validate_transport_ledger(&transfer_primary, true, "partial transfer ledger")?;
    let mut transfer_receipt = transport_admitted(native);
    transfer_receipt["status"] = json!("failed");
    transfer_receipt["stage_ledger"] = transfer_primary.clone();
    let transfer_outcome = json!({
        "terminal": {"status": "failed", "primary": {"kind": "transfer"}, "cleanup_outcome": "cleanup_failed"},
        "receipt": transfer_receipt
    });
    if !transport_outcome_primary_coherent(&transfer_outcome) {
        return Err("earlier transfer failure was overwritten by cleanup".to_owned());
    }
    let mut overwritten = transfer_outcome;
    overwritten["terminal"]["primary"]["kind"] = json!("cleanup");
    if transport_outcome_primary_coherent(&overwritten) {
        return Err("cleanup overwrote an earlier transfer primary failure".to_owned());
    }
    if cleanup_primary["object_count"] != "1" || transfer_primary["object_count"] != "200000" {
        return Err("frozen transport object count changed with reached transfer facts".to_owned());
    }
    if frozen_transport_object_count(FrozenTransportSubject::ProjectMaterialization { files: 7 })
        != "7"
        || frozen_transport_object_count(FrozenTransportSubject::CompilationDelivery {
            artifacts: 0,
        }) != "0"
    {
        return Err("role-specific frozen transport object counts are incoherent".to_owned());
    }
    let complete_cleanup = transport_admitted(native);
    if !transport_cleanup_coherent(&complete_cleanup)
        || !transport_scope_coherent(&complete_cleanup)
    {
        return Err("complete-before-return cleanup fixture is incoherent".to_owned());
    }
    let mut illegal_residual = complete_cleanup.clone();
    illegal_residual["stage_ledger"]["cleanup_outcome"] = json!("residual_reported");
    illegal_residual["stage_ledger"]["residual_locator"] = json!({"safe_summary": "partial"});
    if transport_cleanup_coherent(&illegal_residual) {
        return Err("residual state was accepted under complete-before-return".to_owned());
    }
    let mut permitted_residual = illegal_residual;
    permitted_residual["admission"]["requested_cleanup_requirement"] =
        json!("residual_locator_permitted");
    permitted_residual["admission"]["admitted_cleanup_requirement"] =
        json!("residual_locator_permitted");
    if !transport_cleanup_coherent(&permitted_residual) {
        return Err("permitted residual cleanup state was rejected".to_owned());
    }
    summary.semantic_cases += 3;

    let publication_refusal = archive_publication_refusal(native);
    if !transport_scope_coherent(&publication_refusal["transport"]["receipt"]) {
        return Err("publication transport scope is not bound to its exact subject".to_owned());
    }
    validate_definition(
        schema,
        "format_receipt_request_facts",
        &publication_refusal["format"]["request"],
        true,
        "paired archive publication request facts",
    )?;
    validate_definition(
        schema,
        "publication_transport_admission_refused",
        &publication_refusal["format"]["admission"],
        true,
        "paired archive publication admission refusal",
    )?;
    validate_definition(
        schema,
        "format_receipt",
        &publication_refusal["format"],
        true,
        "paired archive publication format refusal",
    )?;
    validate_definition(
        schema,
        "archive_publication_outcome",
        &publication_refusal,
        true,
        "paired archive publication refusal",
    )?;
    if !publication_refusal_coherent(&publication_refusal) {
        return Err("paired archive publication refusal is incoherent".to_owned());
    }
    let mut mismatched_publication_refusal = publication_refusal.clone();
    mismatched_publication_refusal["format"]["admission"]["reason"] =
        json!("cleanup_requirement_unavailable");
    if publication_refusal_coherent(&mismatched_publication_refusal) {
        return Err("cross-receipt publication reason mutation was accepted".to_owned());
    }
    let closure_refusal = closure_publication_refusal(native);
    validate_definition(
        schema,
        "closure_publication_outcome",
        &closure_refusal,
        true,
        "paired Closure Export publication refusal",
    )?;
    if !closure_publication_refusal_coherent(&closure_refusal) {
        return Err("paired Closure Export publication refusal is incoherent".to_owned());
    }
    let mut mutated_closure = closure_refusal;
    mutated_closure["transport"]["receipt"]["requested_subject"]["source_tree"] =
        json!(identity("closure-export-tree-content", 'f'));
    if closure_publication_refusal_coherent(&mutated_closure) {
        return Err("cross-receipt Closure Export subject mutation was accepted".to_owned());
    }
    summary.semantic_cases += 4;

    let refusal = compilation_admission_refusal(native);
    if refusal["stage"] != "admission"
        || refusal["compilation_identity"].is_null()
        || refusal.get("report").is_some()
    {
        return Err("prepared compilation admission refusal precedence is lossy".to_owned());
    }
    let rejection_beats_jobs = compile_operation_request_rejected(native);
    validate_definition(
        schema,
        "compile_operation",
        &rejection_beats_jobs,
        true,
        "invalid semantic request beats unavailable exact jobs",
    )?;
    if !rejection_beats_jobs["adapter_jobs"]["admitted"].is_null()
        || rejection_beats_jobs.get("admission_refusal").is_some()
        || rejection_beats_jobs.get("report").is_some()
    {
        return Err("request rejection did not precede operational jobs admission".to_owned());
    }
    summary.semantic_cases += 1;

    let creation_refusal = creation_admission_refusal(native);
    let creation_offered = json!([
        creation_refusal["evidence"]["offered_scope"],
        creation_refusal["packages"]["offered_scope"],
        creation_refusal["fonts"]["offered_scope"],
        creation_refusal["reporting"]["offered_scope"]
    ]);
    if !capability_scopes_are_subsets(
        &creation_refusal["operation_request"]["required_capability_scopes"],
        &creation_offered,
        &json!([]),
    ) {
        return Err("creation capability scopes are not sourced from bound descriptors".to_owned());
    }
    let compilation_offered = json!([
        refusal["packages"]["offered_scope"],
        refusal["fonts"]["offered_scope"],
        refusal["reporting"]["offered_scope"]
    ]);
    if !capability_scopes_are_subsets(
        &refusal["operation_request"]["required_capability_scopes"],
        &compilation_offered,
        &json!([]),
    ) {
        return Err(
            "compilation capability scopes are not sourced from bound descriptors".to_owned(),
        );
    }
    let mut duplicate_scopes = refusal["operation_request"]["required_capability_scopes"].clone();
    duplicate_scopes
        .as_array_mut()
        .ok_or("capability scopes must be an array")?
        .push(json!({"role": "reporting", "permitted_uses": ["timing"], "coverage": "selected_report_channels", "completeness": "complete"}));
    if capability_scope_map(&duplicate_scopes).is_some() {
        return Err("duplicate capability role escaped semantic validation".to_owned());
    }
    for reason in [
        "required_capability_scope_unavailable",
        "execution_placement_unavailable",
        "isolation_contract_unavailable",
        "required_reporting_unavailable",
    ] {
        let mut typed_refusal = refusal.clone();
        typed_refusal["reason"] = json!(reason);
        if typed_refusal["stage"] != "admission"
            || typed_refusal.get("report").is_some()
            || typed_refusal["compilation_identity"].is_null()
        {
            return Err(format!(
                "{reason} did not remain a reportless prepared refusal"
            ));
        }
    }
    summary.semantic_cases += 7;

    let materialization = project_materialization_receipt(native);
    let effect = field(&materialization, "effect")?;
    let files = array_at(effect, "/files")?;
    let file_count = parse_decimal(
        string_field(effect, "file_count")?,
        "materialization file_count",
    )?;
    if file_count != files.len() as u128 {
        return Err("project materialization file_count differs from files".to_owned());
    }
    let aggregate = parse_decimal(
        string_field(effect, "aggregate_bytes")?,
        "materialization aggregate",
    )?;
    let sum = files.iter().try_fold(0_u128, |sum, file| {
        parse_decimal(
            string_field(file, "exact_bytes")?,
            "materialization file bytes",
        )
        .map(|value| sum + value)
    })?;
    if aggregate != sum {
        return Err("project materialization aggregate_bytes differs from file sum".to_owned());
    }
    check_eq(
        effect
            .pointer("/common/publication/kind")
            .and_then(Value::as_str),
        Some("not_applicable"),
        "project materialization common publication",
    )?;

    let archive_receipt = archive_encoding_format_receipt(native);
    let accounting = archive_receipt
        .pointer("/effect/common/accounting")
        .ok_or("archive receipt missing accounting")?;
    let control = parse_decimal(
        string_field(&accounting["physical"], "control_record_bytes")?,
        "archive C",
    )?;
    let blobs = parse_decimal(
        string_field(&accounting["physical"], "blob_count")?,
        "archive B",
    )?;
    let blob_bytes = parse_decimal(
        string_field(&accounting["physical"], "blob_bytes")?,
        "archive P",
    )?;
    let entries = parse_decimal(
        string_field(&accounting["physical"], "representation_entries")?,
        "archive N",
    )?;
    let planned = parse_decimal(
        string_field(accounting, "planned_output_bytes")?,
        "archive A",
    )?;
    let expected_archive =
        control + blob_bytes + 138 + 252 * blobs + if entries >= 65_535 { 76 } else { 0 };
    if planned != expected_archive
        || accounting["completed_output_bytes"] != accounting["planned_output_bytes"]
        || archive_receipt["effect"]["closure_export_tree_identity"].is_null()
    {
        return Err(
            "successful archive receipt violates all-Stored plan or identity coherence".to_owned(),
        );
    }
    summary.semantic_cases += 1;

    let mut wrong_role_receipt = project_materialization_receipt(native);
    wrong_role_receipt["role"] = json!("pack_archive_encode");
    wrong_role_receipt["request"] = json!({
        "kind": "pack_archive_encode",
        "source_pack_identity": identity("pack", 'b'),
        "selected_archive_encoding_identity": first_party_archive_encoding_identity()
    });
    validate_definition(
        schema,
        "format_receipt",
        &wrong_role_receipt,
        false,
        "cross-role Format Receipt accounting poison",
    )?;
    let mut impossible_archive_terminal = archive_receipt.clone();
    impossible_archive_terminal["terminal"] = json!("invalid");
    validate_definition(
        schema,
        "pack_archive_encoding_format_receipt",
        &impossible_archive_terminal,
        false,
        "archive encode invalid-terminal poison",
    )?;
    summary.semantic_cases += 1;

    let retiring = session_state("retiring", false);
    check_eq(
        retiring
            .pointer("/active_or_draining/state")
            .and_then(Value::as_str),
        Some("draining"),
        "retiring session attempt state",
    )?;
    let retired = session_state("retired", true);
    if !retired["active_or_draining"].is_null() || retired["last_successful"].is_null() {
        return Err(
            "retired session fixture must have no attempt and retain last success".to_owned(),
        );
    }

    for case in array_at(cases, "/semantic_generated_cases")? {
        let id = string_field(case, "id")?;
        let definition = string_field(case, "definition")?;
        let builder = string_field(case, "builder")?;
        let semantic = string_field(case, "semantic")?;
        let expected = bool_field(case, "valid")?;
        let instance = build_generated_case(builder, native, dagger)?;
        validate_definition(schema, definition, &instance, true, &format!("{id} schema"))?;
        let actual = match semantic {
            "automatic_jobs" => automatic_jobs_width_coherent(&instance),
            "exact_jobs" => exact_jobs_width_coherent(&instance),
            "exact_refusal" => exact_refusal_width_coherent(&instance),
            "session_preparation" => session_preparation_profile_coherent(&instance, native),
            "no_hostile" => !contains_string(&instance, "hostile"),
            "watch_delivery" => watch_delivery_coherent(&instance),
            other => return Err(format!("{id}: unknown semantic validator `{other}`")),
        };
        if actual != expected {
            return Err(format!(
                "{id}: expected semantic validity {expected}, got {actual}"
            ));
        }
        summary.semantic_cases += 1;
    }

    assert_first_party_cache_state(native)?;

    for case in array_at(cases, "/generated_schema_cases")? {
        if bool_field(case, "valid")? {
            let builder = string_field(case, "builder")?;
            let instance = build_generated_case(builder, native, dagger)?;
            if contains_string(&instance, "hostile") {
                return Err(format!(
                    "{}: schema-positive inner first-party fixture contains Hostile",
                    string_field(case, "id")?
                ));
            }
        }
    }
    Ok(())
}

fn automatic_jobs_width_coherent(operation: &Value) -> bool {
    let lexical = operation
        .pointer("/adapter_jobs/lexical")
        .and_then(Value::as_str);
    let normalized = operation
        .pointer("/adapter_jobs/normalized/kind")
        .and_then(Value::as_str);
    let requested = operation
        .pointer("/report/operational_inventory/role_execution/engine_width/requested/kind")
        .and_then(Value::as_str);
    let admitted_jobs = operation
        .pointer("/adapter_jobs/admitted")
        .and_then(Value::as_str);
    let admitted_width = operation
        .pointer("/report/operational_inventory/role_execution/engine_width/admitted")
        .and_then(Value::as_str);
    let domain_width = operation
        .pointer("/report/operational_inventory/role_execution/domain/width")
        .and_then(Value::as_str);
    let domain_kind = operation
        .pointer("/report/operational_inventory/role_execution/domain/kind")
        .and_then(Value::as_str);
    matches!(
        lexical,
        Some("omitted_automatic" | "explicit_zero_automatic")
    ) && normalized == Some("automatic")
        && requested == Some("automatic")
        && admitted_jobs.is_some()
        && admitted_jobs == admitted_width
        && (domain_kind == Some("not_selected") && domain_width.is_none()
            || domain_kind == Some("managed") && admitted_width == domain_width)
}

fn exact_jobs_width_coherent(operation: &Value) -> bool {
    let widths = [
        "/adapter_jobs/normalized/width",
        "/report/operational_inventory/role_execution/engine_width/requested/width",
        "/adapter_jobs/admitted",
        "/report/operational_inventory/role_execution/engine_width/admitted",
        "/report/operational_inventory/role_execution/domain/width",
    ]
    .map(|pointer| operation.pointer(pointer).and_then(Value::as_str));
    operation
        .pointer("/adapter_jobs/lexical")
        .and_then(Value::as_str)
        == Some("exact_positive")
        && operation
            .pointer("/adapter_jobs/normalized/kind")
            .and_then(Value::as_str)
            == Some("exact")
        && operation
            .pointer("/report/operational_inventory/role_execution/engine_width/requested/kind")
            .and_then(Value::as_str)
            == Some("exact")
        && widths[0].is_some()
        && widths.iter().all(|width| *width == widths[0])
}

fn exact_refusal_width_coherent(operation: &Value) -> bool {
    let normalized = operation
        .pointer("/adapter_jobs/normalized/width")
        .and_then(Value::as_str);
    let requested = operation
        .pointer("/admission_refusal/operation_request/engine_width/width")
        .and_then(Value::as_str);
    operation
        .pointer("/adapter_jobs/lexical")
        .and_then(Value::as_str)
        == Some("exact_positive")
        && operation["adapter_jobs"]["admitted"].is_null()
        && operation
            .pointer("/admission_refusal/operation_request/engine_width/kind")
            .and_then(Value::as_str)
            == Some("exact")
        && normalized.is_some()
        && normalized == requested
}

fn session_preparation_profile_coherent(publication: &Value, profile: &Value) -> bool {
    let preparation = publication.pointer("/ingestion_failure/policy/preparation");
    preparation
        .and_then(|value| value.get("origin"))
        .and_then(Value::as_str)
        == Some("adapter_profile")
        && preparation.and_then(|value| value.get("resource_profile")) == Some(&profile_reference())
        && preparation.and_then(|value| value.get("policy"))
            == profile.pointer("/compilation_preparation/policy")
        && preparation.and_then(|value| value.get("limits"))
            == profile.pointer("/compilation_preparation/limits")
}

fn contains_string(value: &Value, needle: &str) -> bool {
    match value {
        Value::Object(object) => object.values().any(|child| contains_string(child, needle)),
        Value::Array(values) => values.iter().any(|child| contains_string(child, needle)),
        Value::String(text) => text == needle,
        _ => false,
    }
}

fn watch_delivery_coherent(delivery: &Value) -> bool {
    let session = delivery.get("session_instance");
    let revision = delivery.get("revision");
    let evaluation = delivery.get("evaluation");
    let publication = delivery.get("publication_sequence");
    let result = delivery.get("result_identity");
    session.is_some()
        && revision.and_then(|value| value.get("session_instance")) == session
        && evaluation.and_then(|value| value.get("session_instance")) == session
        && evaluation.and_then(|value| value.get("revision")) == revision
        && publication.and_then(|value| value.get("session_instance")) == session
        && delivery.pointer("/outcome/report/report_projection/result_identity") == result
        && delivery.pointer("/outcome/transport/receipt/subject/result") == result
        && delivery.pointer("/outcome/report/report_projection/compilation_identity")
            == delivery.pointer("/outcome/transport/receipt/subject/compilation")
}

fn assert_first_party_cache_state(native: &Value) -> CheckResult<()> {
    let request = compilation_operation_request();
    check_eq(
        request.pointer("/cache/kind").and_then(Value::as_str),
        Some("disabled"),
        "first-party compilation request cache",
    )?;
    let refusal = compilation_admission_refusal(native);
    if !refusal["cache"].is_null()
        || refusal
            .pointer("/operation_request/cache/kind")
            .and_then(Value::as_str)
            != Some("disabled")
    {
        return Err("first-party compilation refusal cache is not disabled/null".to_owned());
    }
    let inventory = compilation_operational_inventory(native);
    if !inventory["dependency_execution"]["cache_descriptor"].is_null()
        || inventory["dependency_execution"]["cache_policy"]["kind"] != "disabled"
        || inventory["dependency_execution"]["cache_lookup"] != "disabled"
        || inventory["dependency_execution"]["cache_isolation_domain_present"] != false
    {
        return Err(
            "first-party compilation inventory cache state is not disabled/null/false".to_owned(),
        );
    }
    let report = compilation_report(native, false);
    if report["cache_provenance"] != cache_provenance() {
        return Err("first-party report cache provenance is not disabled/none".to_owned());
    }
    Ok(())
}

fn validate_transport_ledger(ledger: &Value, expected: bool, label: &str) -> CheckResult<()> {
    let order = [
        "admission",
        "plan_freeze",
        "reference_resolution",
        "acquisition",
        "spooling",
        "transfer",
        "verification",
        "commit",
        "cleanup",
        "complete",
    ];
    let rank = order
        .iter()
        .enumerate()
        .map(|(index, stage)| (*stage, index))
        .collect::<BTreeMap<_, _>>();
    let stages = strings_at(ledger, "/stages")?;
    let primary = ledger.get("primary_terminal").and_then(Value::as_str);
    let increasing = stages.windows(2).all(|pair| rank[pair[0]] < rank[pair[1]])
        && stages.first().copied() == Some("admission")
        && stages.get(1).copied() == Some("plan_freeze")
        && primary.is_some_and(|stage| stages.contains(&stage));
    let commit_coherent = ledger["actual_commit"].is_null() || stages.contains(&"commit");
    let cleanup_primary_coherent =
        primary != Some("cleanup") || ledger["cleanup_outcome"] == "cleanup_failed";
    let committed_primary_coherent = ledger["actual_commit"].is_null()
        || matches!(primary, Some("commit" | "cleanup" | "complete"));
    let actual =
        increasing && commit_coherent && cleanup_primary_coherent && committed_primary_coherent;
    if actual != expected {
        return Err(format!(
            "{label}: expected semantic validity {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn transport_cleanup_coherent(receipt: &Value) -> bool {
    let requirement = receipt
        .pointer("/admission/admitted_cleanup_requirement")
        .and_then(Value::as_str);
    let outcome = receipt
        .pointer("/stage_ledger/cleanup_outcome")
        .and_then(Value::as_str);
    let residual = receipt.pointer("/stage_ledger/residual_locator");
    match (requirement, outcome, residual) {
        (_, Some("complete" | "not_required"), Some(Value::Null)) => true,
        (Some("residual_locator_permitted"), Some("residual_reported"), Some(Value::Object(_))) => {
            true
        }
        (Some("complete_before_return"), Some("cleanup_failed"), _) => true,
        (Some("residual_locator_permitted"), Some("cleanup_failed"), _) => true,
        _ => false,
    }
}

fn transport_scope_coherent(receipt: &Value) -> bool {
    if receipt.get("status").and_then(Value::as_str) == Some("admission_refused") {
        return capability_scopes_are_subsets(
            &json!([receipt["request"]["required_scope"]]),
            &json!([receipt["descriptor"]["offered_scope"]]),
            &json!([]),
        );
    }
    capability_scopes_are_subsets(
        &json!([receipt["admission"]["requested_scope"]]),
        &json!([receipt["admission"]["admitted_scope"]]),
        &json!([receipt["stage_ledger"]["reached_scope"]]),
    ) && capability_scopes_are_subsets(
        &json!([receipt["admission"]["requested_scope"]]),
        &json!([receipt["admission"]["descriptor"]["offered_scope"]]),
        &json!([]),
    )
}

fn transport_outcome_primary_coherent(outcome: &Value) -> bool {
    let receipt_stage = outcome
        .pointer("/receipt/stage_ledger/primary_terminal")
        .and_then(Value::as_str);
    match outcome.pointer("/terminal/status").and_then(Value::as_str) {
        Some("succeeded") => receipt_stage == Some("complete"),
        Some("admission_refused") => {
            outcome.pointer("/receipt/status").and_then(Value::as_str) == Some("admission_refused")
                && receipt_stage.is_none()
        }
        Some("failed") => {
            let expected_stage = match outcome
                .pointer("/terminal/primary/kind")
                .and_then(Value::as_str)
            {
                Some("acquisition") => "acquisition",
                Some("transfer") => "transfer",
                Some("commit") => "commit",
                Some("cleanup") => "cleanup",
                Some("cancelled" | "deadline") => return true,
                _ => return false,
            };
            receipt_stage == Some(expected_stage)
        }
        _ => false,
    }
}

fn validate_graphql(source: &str, summary: &mut Summary) -> CheckResult<()> {
    let document =
        parse_schema::<String>(source).map_err(|error| format!("parse GraphQL SDL: {error}"))?;
    let objects = graphql_objects(&document);
    let enums = graphql_enums(&document);
    summary.graphql_types = document
        .definitions
        .iter()
        .filter(|definition| matches!(definition, Definition::TypeDefinition(_)))
        .count();

    for name in objects.keys().chain(enums.keys()) {
        let lower = name.to_ascii_lowercase();
        if lower.contains("watch") || lower.contains("session") {
            return Err(format!(
                "GraphQL exposes forbidden watch/session type `{name}`"
            ));
        }
    }

    assert_graphql_fields(
        &objects,
        "Query",
        &[
            "with",
            "discoveryVariant",
            "annotation",
            "reportDisclosure",
            "pdf",
            "png",
            "svg",
            "html",
            "create",
            "readArchive",
            "readClosure",
        ],
    )?;
    if graphql_field(&objects, "Query", "typstPack").is_ok() {
        return Err("GraphQL must not expose Query.typstPack".to_owned());
    }
    assert_non_null_named(
        &graphql_field(&objects, "Query", "create")?.field_type,
        "TypstPackPackCreation",
        "Query.create",
    )?;
    assert_non_null_named(
        &graphql_field(&objects, "TypstPackPack", "compile")?.field_type,
        "TypstPackCompilation",
        "TypstPackPack.compile",
    )?;
    assert_nullable_argument(&objects, "Query", "create", "jobs", "Int")?;
    assert_nullable_argument(&objects, "TypstPackPack", "compile", "jobs", "Int")?;

    for (object, field, arguments) in [
        (
            "Query",
            "with",
            &["authorityCache", "authorityCacheQuotaBytes"][..],
        ),
        (
            "Query",
            "discoveryVariant",
            &[
                "label",
                "target",
                "sysInputs",
                "features",
                "documentTime",
                "overrides",
            ],
        ),
        (
            "Query",
            "annotation",
            &["identifier", "annotationEpoch", "payload"],
        ),
        (
            "Query",
            "reportDisclosure",
            &[
                "diagnostics",
                "evidence",
                "sources",
                "requestValues",
                "overrideBytes",
                "backingLocators",
                "adapterDetail",
            ],
        ),
        (
            "Query",
            "pdf",
            &[
                "pages",
                "identifierMode",
                "identifier",
                "creatorMode",
                "creator",
                "creationTime",
                "standards",
                "tagging",
                "pretty",
            ],
        ),
        ("Query", "png", &["pages", "ppi", "bleed"]),
        ("Query", "svg", &["pages", "bleed", "pretty"]),
        ("Query", "html", &["pretty"]),
        (
            "Query",
            "create",
            &[
                "project",
                "input",
                "variants",
                "target",
                "sysInputs",
                "features",
                "documentTime",
                "discoveryOverrides",
                "include",
                "packageDirectories",
                "fontDirectories",
                "packageEmbedding",
                "fontEmbedding",
                "embeddingPolicy",
                "title",
                "description",
                "authors",
                "keywords",
                "annotations",
                "offline",
                "certificate",
                "trust",
                "limits",
                "deadline",
                "jobs",
            ],
        ),
        (
            "Query",
            "readArchive",
            &[
                "archive",
                "expectedPackIdentity",
                "expectedArchiveContentIdentity",
                "expectedArchiveEncodingIdentity",
                "trust",
                "limits",
                "deadline",
            ],
        ),
        (
            "Query",
            "readClosure",
            &[
                "closure",
                "expectedPackIdentity",
                "trust",
                "limits",
                "deadline",
            ],
        ),
        (
            "TypstPackPack",
            "archive",
            &["epoch", "encoding", "trust", "limits", "deadline"],
        ),
        (
            "TypstPackPack",
            "closureExport",
            &["epoch", "trust", "limits", "deadline"],
        ),
        (
            "TypstPackPack",
            "materialize",
            &["trust", "limits", "deadline"],
        ),
        (
            "TypstPackPack",
            "compile",
            &[
                "output",
                "overrides",
                "sysInputs",
                "features",
                "documentTime",
                "packageDirectories",
                "fontDirectories",
                "offline",
                "certificate",
                "diagnosticPolicy",
                "reportDisclosure",
                "trust",
                "limits",
                "deadline",
                "jobs",
            ],
        ),
    ] {
        assert_graphql_arguments(&objects, object, field, arguments)?;
    }

    for (object, field, argument, signature) in [
        ("Query", "with", "authorityCache", "ID"),
        ("Query", "discoveryVariant", "sysInputs", "[String!]"),
        ("Query", "discoveryVariant", "features", "[String!]"),
        ("Query", "annotation", "identifier", "String!"),
        ("Query", "annotation", "annotationEpoch", "String!"),
        ("Query", "annotation", "payload", "ID!"),
        ("Query", "pdf", "standards", "[TypstPackPdfStandard!]"),
        ("Query", "create", "project", "ID!"),
        ("Query", "create", "variants", "[ID!]"),
        ("Query", "create", "include", "[String!]"),
        ("Query", "create", "packageDirectories", "[ID!]"),
        ("Query", "create", "fontDirectories", "[ID!]"),
        ("Query", "create", "annotations", "[ID!]"),
        ("Query", "create", "certificate", "ID"),
        ("Query", "readArchive", "archive", "ID!"),
        ("Query", "readClosure", "closure", "ID!"),
        ("TypstPackPack", "compile", "output", "ID!"),
        ("TypstPackPack", "compile", "packageDirectories", "[ID!]"),
        ("TypstPackPack", "compile", "fontDirectories", "[ID!]"),
        ("TypstPackPack", "compile", "jobs", "Int"),
    ] {
        assert_graphql_argument_type(&objects, object, field, argument, signature)?;
    }

    for (object, expected) in [
        ("TypstPackAdmissionRefusal", &["stage"][..]),
        (
            "TypstPackPackCreation",
            &[
                "id",
                "status",
                "admissionRefusal",
                "report",
                "pack",
                "requirePack",
            ][..],
        ),
        (
            "TypstPackPackIngress",
            &[
                "id",
                "status",
                "admissionRefusal",
                "receipt",
                "pack",
                "requirePack",
            ],
        ),
        (
            "TypstPackPack",
            &[
                "id",
                "identity",
                "inspect",
                "archive",
                "closureExport",
                "materialize",
                "compile",
            ],
        ),
        (
            "TypstPackPackArchiveEncoding",
            &[
                "id",
                "status",
                "admissionRefusal",
                "receipt",
                "stagingStatus",
                "stagingReceipt",
                "publicationFormatReceipt",
                "publicationTransportReceipt",
                "archive",
                "requireArchive",
            ],
        ),
        (
            "TypstPackClosureExport",
            &[
                "id",
                "status",
                "admissionRefusal",
                "receipt",
                "stagingStatus",
                "stagingReceipt",
                "publicationFormatReceipt",
                "publicationTransportReceipt",
                "tree",
                "requireTree",
            ],
        ),
        (
            "TypstPackProjectMaterialization",
            &[
                "id",
                "status",
                "admissionRefusal",
                "receipt",
                "stagingStatus",
                "stagingReceipt",
                "project",
                "requireProject",
            ],
        ),
        (
            "TypstPackCompilation",
            &[
                "id",
                "status",
                "admissionRefusal",
                "operation",
                "terminal",
                "report",
                "compilationIdentity",
                "resultIdentity",
                "stagingStatus",
                "stagingReceipt",
                "artifacts",
                "requireSuccess",
            ],
        ),
    ] {
        assert_graphql_fields(&objects, object, expected)?;
    }

    for (object, field, signature) in [
        (
            "TypstPackAdmissionRefusal",
            "stage",
            "TypstPackAdmissionStage!",
        ),
        (
            "TypstPackPackCreation",
            "status",
            "TypstPackCreationStatus!",
        ),
        ("TypstPackPackCreation", "report", "File!"),
        (
            "TypstPackPackIngress",
            "status",
            "TypstPackPackIngressStatus!",
        ),
        ("TypstPackPackIngress", "receipt", "File!"),
        (
            "TypstPackPackArchiveEncoding",
            "status",
            "TypstPackRepresentationStatus!",
        ),
        ("TypstPackPackArchiveEncoding", "receipt", "File!"),
        (
            "TypstPackClosureExport",
            "status",
            "TypstPackRepresentationStatus!",
        ),
        ("TypstPackClosureExport", "receipt", "File!"),
        (
            "TypstPackProjectMaterialization",
            "status",
            "TypstPackRepresentationStatus!",
        ),
        ("TypstPackProjectMaterialization", "receipt", "File!"),
        (
            "TypstPackCompilation",
            "status",
            "TypstPackCompilationStatus!",
        ),
        ("TypstPackCompilation", "operation", "File!"),
        ("TypstPackCompilation", "terminal", "File"),
        ("TypstPackCompilation", "report", "File"),
        (
            "TypstPackPackCreation",
            "admissionRefusal",
            "TypstPackAdmissionRefusal",
        ),
        (
            "TypstPackPackIngress",
            "admissionRefusal",
            "TypstPackAdmissionRefusal",
        ),
        (
            "TypstPackPackArchiveEncoding",
            "admissionRefusal",
            "TypstPackAdmissionRefusal",
        ),
        (
            "TypstPackClosureExport",
            "admissionRefusal",
            "TypstPackAdmissionRefusal",
        ),
        (
            "TypstPackProjectMaterialization",
            "admissionRefusal",
            "TypstPackAdmissionRefusal",
        ),
        (
            "TypstPackCompilation",
            "admissionRefusal",
            "TypstPackAdmissionRefusal",
        ),
        (
            "TypstPackPackArchiveEncoding",
            "publicationFormatReceipt",
            "File",
        ),
        (
            "TypstPackPackArchiveEncoding",
            "publicationTransportReceipt",
            "File",
        ),
        ("TypstPackClosureExport", "publicationFormatReceipt", "File"),
        (
            "TypstPackClosureExport",
            "publicationTransportReceipt",
            "File",
        ),
    ] {
        assert_graphql_field_type(&objects, object, field, signature)?;
    }

    for required_comment in [
        "operational admission refusal before any Compilation Report exists",
        "Null for\n  # adapter failure and operational admission refusal",
        "Null unless terminal is the Compilation Report branch",
        "Creation Admission Refusal, and admitted Creation Report branches",
    ] {
        if !source.contains(required_comment) {
            return Err(format!(
                "GraphQL fixture lacks admission-refusal/nullability contract text `{required_comment}`"
            ));
        }
    }
    if !source.contains("not evidence that implementation source or generated bindings exist") {
        return Err("GraphQL fixture does not identify generated parity as deferred".to_owned());
    }

    for (object, nullable, required, named) in [
        (
            "TypstPackPackCreation",
            "pack",
            "requirePack",
            "TypstPackPack",
        ),
        (
            "TypstPackPackIngress",
            "pack",
            "requirePack",
            "TypstPackPack",
        ),
        (
            "TypstPackPackArchiveEncoding",
            "archive",
            "requireArchive",
            "File",
        ),
        ("TypstPackClosureExport", "tree", "requireTree", "Directory"),
        (
            "TypstPackProjectMaterialization",
            "project",
            "requireProject",
            "Directory",
        ),
        (
            "TypstPackCompilation",
            "artifacts",
            "requireSuccess",
            "Directory",
        ),
    ] {
        assert_nullable_named(
            &graphql_field(&objects, object, nullable)?.field_type,
            named,
            &format!("{object}.{nullable}"),
        )?;
        assert_non_null_named(
            &graphql_field(&objects, object, required)?.field_type,
            named,
            &format!("{object}.{required}"),
        )?;
    }

    for (name, values) in [
        ("TypstPackAdmissionStage", &["ADMISSION"][..]),
        (
            "TypstPackCreationStatus",
            &[
                "ADAPTER_FAILED",
                "REQUEST_REJECTED",
                "ADMISSION_REFUSED",
                "CREATION_FAILED",
                "PACK_ISSUED",
            ][..],
        ),
        (
            "TypstPackPackIngressStatus",
            &[
                "ADAPTER_FAILED",
                "VALIDATED",
                "INVALID",
                "UNSUPPORTED",
                "EXPECTED_PACK_IDENTITY_MISMATCH",
                "INPUT_CONTENT_IDENTITY_MISMATCH",
                "ARCHIVE_ENCODING_ASSERTION_MISMATCH",
                "RESOURCE_LIMIT",
                "CANCELLED",
                "DEADLINE",
                "ADMISSION_REFUSED",
                "INTERNAL_INTEGRITY",
            ],
        ),
        (
            "TypstPackRepresentationStatus",
            &[
                "ADAPTER_FAILED",
                "SUCCEEDED",
                "ENCODING_FAILED",
                "SPOOLING_FAILED",
                "RESOURCE_LIMIT",
                "CANCELLED",
                "DEADLINE",
                "ADMISSION_REFUSED",
                "INTERNAL_INTEGRITY",
            ],
        ),
        (
            "TypstPackCompilationStatus",
            &[
                "ADAPTER_FAILED",
                "REQUEST_REJECTED",
                "ADMISSION_REFUSED",
                "RESULT_SUCCEEDED",
                "RESULT_REJECTED",
                "OPERATION_FAILED",
            ],
        ),
        (
            "TypstPackStagingStatus",
            &["NOT_APPLICABLE", "SUCCEEDED", "FAILED"],
        ),
    ] {
        let actual = enums
            .get(name)
            .ok_or_else(|| format!("missing GraphQL enum `{name}`"))?;
        let expected = values
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        if actual != &expected {
            return Err(format!(
                "GraphQL enum `{name}` differs: expected {expected:?}, got {actual:?}"
            ));
        }
    }
    Ok(())
}

fn graphql_objects<'a>(
    document: &'a Document<'a, String>,
) -> BTreeMap<&'a str, &'a [Field<'a, String>]> {
    document
        .definitions
        .iter()
        .filter_map(|definition| match definition {
            Definition::TypeDefinition(TypeDefinition::Object(object)) => {
                Some((object.name.as_str(), object.fields.as_slice()))
            }
            _ => None,
        })
        .collect()
}

fn graphql_enums<'a>(document: &'a Document<'a, String>) -> BTreeMap<&'a str, Vec<String>> {
    document
        .definitions
        .iter()
        .filter_map(|definition| match definition {
            Definition::TypeDefinition(TypeDefinition::Enum(value)) => Some((
                value.name.as_str(),
                value.values.iter().map(|item| item.name.clone()).collect(),
            )),
            _ => None,
        })
        .collect()
}

fn graphql_field<'a>(
    objects: &BTreeMap<&str, &'a [Field<'a, String>]>,
    object: &str,
    field: &str,
) -> CheckResult<&'a Field<'a, String>> {
    objects
        .get(object)
        .ok_or_else(|| format!("missing GraphQL object `{object}`"))?
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| format!("missing GraphQL field `{object}.{field}`"))
}

fn assert_graphql_fields(
    objects: &BTreeMap<&str, &[Field<'_, String>]>,
    object: &str,
    expected: &[&str],
) -> CheckResult<()> {
    let actual = objects
        .get(object)
        .ok_or_else(|| format!("missing GraphQL object `{object}`"))?
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(format!(
            "GraphQL `{object}` fields differ: expected {expected:?}, got {actual:?}"
        ));
    }
    Ok(())
}

fn assert_graphql_arguments<'a>(
    objects: &BTreeMap<&str, &'a [Field<'a, String>]>,
    object: &str,
    field: &str,
    expected: &[&str],
) -> CheckResult<()> {
    let actual = graphql_field(objects, object, field)?
        .arguments
        .iter()
        .map(|argument| argument.name.as_str())
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(format!(
            "GraphQL `{object}.{field}` arguments differ: expected {expected:?}, got {actual:?}"
        ));
    }
    Ok(())
}

fn assert_graphql_field_type<'a>(
    objects: &BTreeMap<&str, &'a [Field<'a, String>]>,
    object: &str,
    field: &str,
    expected: &str,
) -> CheckResult<()> {
    let actual = graphql_type_signature(&graphql_field(objects, object, field)?.field_type);
    if actual != expected {
        return Err(format!(
            "GraphQL `{object}.{field}` type differs: expected `{expected}`, got `{actual}`"
        ));
    }
    Ok(())
}

fn assert_graphql_argument_type<'a>(
    objects: &BTreeMap<&str, &'a [Field<'a, String>]>,
    object: &str,
    field: &str,
    argument: &str,
    expected: &str,
) -> CheckResult<()> {
    let field = graphql_field(objects, object, field)?;
    let argument = field
        .arguments
        .iter()
        .find(|candidate| candidate.name == argument)
        .ok_or_else(|| {
            format!(
                "missing GraphQL argument `{object}.{}({argument}:)`",
                field.name
            )
        })?;
    let actual = graphql_type_signature(&argument.value_type);
    if actual != expected {
        return Err(format!(
            "GraphQL `{object}.{}({}:)` type differs: expected `{expected}`, got `{actual}`",
            field.name, argument.name
        ));
    }
    Ok(())
}

fn graphql_type_signature(value: &Type<'_, String>) -> String {
    match value {
        Type::NamedType(name) => name.clone(),
        Type::ListType(inner) => format!("[{}]", graphql_type_signature(inner)),
        Type::NonNullType(inner) => format!("{}!", graphql_type_signature(inner)),
    }
}

fn assert_nullable_argument<'a>(
    objects: &BTreeMap<&str, &'a [Field<'a, String>]>,
    object: &str,
    field: &str,
    argument: &str,
    named: &str,
) -> CheckResult<()> {
    let field = graphql_field(objects, object, field)?;
    let argument = field
        .arguments
        .iter()
        .find(|candidate| candidate.name == argument)
        .ok_or_else(|| {
            format!(
                "missing GraphQL argument `{object}.{}({argument}:)`",
                field.name
            )
        })?;
    assert_nullable_named(
        &argument.value_type,
        named,
        &format!("{object}.{}({argument}:)", field.name),
    )
}

fn assert_nullable_named(value: &Type<'_, String>, expected: &str, label: &str) -> CheckResult<()> {
    match value {
        Type::NamedType(name) if name == expected => Ok(()),
        _ => Err(format!(
            "{label} must be nullable `{expected}`, got {value:?}"
        )),
    }
}

fn assert_non_null_named(value: &Type<'_, String>, expected: &str, label: &str) -> CheckResult<()> {
    match value {
        Type::NonNullType(inner) if matches!(inner.as_ref(), Type::NamedType(name) if name == expected) => {
            Ok(())
        }
        _ => Err(format!(
            "{label} must be non-null `{expected}!`, got {value:?}"
        )),
    }
}

fn validate_html(source: &str, cases: &Value, summary: &mut Summary) -> CheckResult<()> {
    let metadata_text = between(
        source,
        "<script id=\"fixture-metadata\" type=\"application/json\">",
        "</script>",
        "HTML fixture metadata",
    )?;
    let metadata = parse_json_bytes(
        metadata_text.trim().as_bytes(),
        "HTML fixture metadata",
        false,
    )?;
    check_eq(
        metadata.get("prototype_kind").and_then(Value::as_str),
        Some("throwaway-trace-driven-logic"),
        "HTML prototype kind",
    )?;

    let expected_events = [
        "Accept",
        "DependencyChanged",
        "NotificationGap",
        "Refresh",
        "Retry",
        "AttemptFinished",
        "AttemptAdmissionRefused",
        "AttemptReleased",
        "FenceReadFinished",
        "SubscriptionsArmed",
        "FenceConfirmed",
        "Shutdown",
    ];
    let expected_covered_events = [
        "Accept",
        "Retry",
        "AttemptFinished",
        "AttemptAdmissionRefused",
        "AttemptReleased",
        "FenceReadFinished",
        "SubscriptionsArmed",
        "FenceConfirmed",
        "Shutdown",
    ];
    let expected_effects = [
        "StartAttempt",
        "InterruptAttempt",
        "ReadFence",
        "ArmSubscriptions",
        "ConfirmFence",
        "RetireSubscriptions",
        "Publish",
    ];
    assert_string_array(
        &metadata,
        "/allowed_event_names",
        &expected_events,
        "HTML allowed event names",
    )?;
    assert_string_array(
        &metadata,
        "/covered_event_names",
        &expected_covered_events,
        "HTML covered event names",
    )?;
    assert_string_array(
        &metadata,
        "/allowed_effect_names",
        &expected_effects,
        "HTML allowed effect names",
    )?;
    summary.allowed_events = expected_events.len();
    summary.covered_events = expected_covered_events.len();
    summary.allowed_effects = expected_effects.len();

    let required_scenarios = strings_at(cases, "/html_contract/required_scenario_ids")?;
    let metadata_scenarios = strings_at(&metadata, "/required_scenario_ids")?;
    if metadata_scenarios != required_scenarios {
        return Err(format!(
            "HTML required scenarios differ: expected {required_scenarios:?}, got {metadata_scenarios:?}"
        ));
    }
    for scenario in &required_scenarios {
        if !source.contains(&format!("id: \"{scenario}\"")) {
            return Err(format!("HTML trace body is missing scenario `{scenario}`"));
        }
    }
    summary.scenarios = required_scenarios.len();

    let required_fields = [
        "session_instance",
        "pack_identity",
        "lifecycle",
        "latest_revision",
        "latest_evaluation",
        "active_or_draining_attempt",
        "latest_pending_prepared_revision",
        "candidate",
        "fence",
        "subscription_generation",
        "publication",
        "last_successful_compilation",
        "delivery",
        "last_delivery",
        "emitted_effects",
        "ignored_stale_input",
        "source_ownership",
    ];
    assert_string_array(
        &metadata,
        "/required_state_fields",
        &required_fields,
        "HTML state fields",
    )?;
    for field in required_fields {
        if !source.contains(&format!("{field}:")) {
            return Err(format!("HTML state snapshots do not expose `{field}`"));
        }
    }

    let actual_events = called_tokens(source, "event(\"")?;
    let actual_effects = called_tokens(source, "effect(\"")?;
    for token in &actual_events {
        if !expected_events.contains(&token.as_str()) {
            return Err(format!("HTML uses undeclared event token `{token}`"));
        }
    }
    for token in &actual_effects {
        if !expected_effects.contains(&token.as_str()) {
            return Err(format!("HTML uses undeclared effect token `{token}`"));
        }
    }
    let covered = expected_covered_events
        .iter()
        .map(|token| (*token).to_owned())
        .collect::<BTreeSet<_>>();
    if actual_events != covered {
        return Err(format!(
            "HTML trace-covered events differ from metadata: expected {covered:?}, got {actual_events:?}"
        ));
    }
    let forbidden = strings_at(cases, "/html_contract/forbidden_protocol_tokens")?;
    for token in forbidden {
        if actual_events.contains(token) || actual_effects.contains(token) {
            return Err(format!("HTML uses obsolete protocol token `{token}`"));
        }
    }

    let executable = source
        .split_once("<script>")
        .map(|(_, script)| script)
        .ok_or_else(|| "HTML is missing executable fixture script".to_owned())?;
    for forbidden in [
        "function reducer",
        "const reducer",
        "function reduce",
        "const reduce",
        ".reduce(",
        "function applyTransition",
        "const applyTransition",
        "function transition",
        "const transition",
    ] {
        if executable.contains(forbidden) {
            return Err(format!(
                "HTML contains an independent reducer marker `{forbidden}`"
            ));
        }
    }
    for lifecycle in ["Running", "Retiring", "Retired"] {
        if !executable.contains(&format!("\"{lifecycle}\"")) {
            return Err(format!("HTML traces omit lifecycle `{lifecycle}`"));
        }
    }
    for evidence in [
        "preparation: \"RequestRejected\"",
        "const affectedScopes = (requestSources = [], dependencies = []) => ({ request_sources: requestSources, dependencies });",
        "const currentPush = observations => ({ kind: \"CurrentThroughPush\", observations: providerObservations(observations), uncovered: null, dirty: null });",
        "const currentPoll = observations => ({ kind: \"CurrentAsOfPoll\", observations: providerObservations(observations), uncovered: null, dirty: null });",
        "const stale = scopes => ({ kind: \"Stale\", observations: null, uncovered: null, dirty: scopes });",
        "lastSuccess: success(",
    ] {
        if !executable.contains(evidence) {
            return Err(format!("HTML traces omit required evidence `{evidence}`"));
        }
    }
    if executable.contains("stale([")
        || executable.contains("uncovered: [")
        || executable.contains("dirty: [")
    {
        return Err("HTML contains a lossy affected-scope/currentness array".to_owned());
    }
    for segment in executable.split("stale(").skip(1) {
        if !segment.trim_start().starts_with("affectedScopes(") {
            return Err("HTML stale currentness does not use split affectedScopes".to_owned());
        }
    }

    let trace_steps = extract_js_calls(executable, "traceStep(")?;
    let mut publish_count = 0;
    for index in 0..trace_steps.len() {
        if called_tokens(&trace_steps[index], "effect(\"")?.contains("Publish") {
            if index < 2 {
                return Err("HTML Publish has no complete preceding fence sequence".to_owned());
            }
            assert_trace_step(
                &trace_steps[index - 2],
                "FenceReadFinished",
                "ArmSubscriptions",
                "publishing fence read",
            )?;
            assert_trace_step(
                &trace_steps[index - 1],
                "SubscriptionsArmed",
                "ConfirmFence",
                "publishing subscription arm",
            )?;
            assert_trace_step(
                &trace_steps[index],
                "FenceConfirmed",
                "Publish",
                "publishing confirmation",
            )?;
            publish_count += 1;
        }
    }
    if publish_count == 0 {
        return Err("HTML has no publishing trace sequence".to_owned());
    }
    summary.publishing_sequences = publish_count;

    assert_observation_trace(
        &trace_steps,
        "read-zero-observations",
        "FenceReadFinished",
        "ArmSubscriptions",
        &["readOutcome({ observations: [] })"],
    )?;
    assert_observation_trace(
        &trace_steps,
        "arm-zero-observations",
        "SubscriptionsArmed",
        "ConfirmFence",
        &["armedOutcome(\"CompletePoll\", [])"],
    )?;
    assert_observation_trace(
        &trace_steps,
        "publish-zero-observations",
        "FenceConfirmed",
        "Publish",
        &["cleanOutcome([])", "currentPoll([])"],
    )?;
    let multiple = "[\"project-provider@31\", \"metadata-provider@9\"]";
    assert_observation_trace(
        &trace_steps,
        "read-multiple-observations",
        "FenceReadFinished",
        "ArmSubscriptions",
        &[multiple],
    )?;
    assert_observation_trace(
        &trace_steps,
        "arm-multiple-observations",
        "SubscriptionsArmed",
        "ConfirmFence",
        &["armedOutcome(\"CompletePoll\"", multiple],
    )?;
    assert_observation_trace(
        &trace_steps,
        "publish-multiple-observations",
        "FenceConfirmed",
        "Publish",
        &["cleanOutcome(", multiple, "currentPoll("],
    )?;
    let refusal_step = trace_steps
        .iter()
        .find(|step| step.contains("id: \"admission-refusal-clear-token\""))
        .ok_or("HTML lacks exact attempt-admission-refusal trace step")?;
    if !refusal_step.contains("event(\"AttemptAdmissionRefused\", { bound_token: \"a1\"")
        || !refusal_step.contains("effect(\"StartAttempt\", { token: \"a2\"")
        || refusal_step.contains("event(\"AttemptFinished\"")
        || refusal_step.contains("effect(\"ReadFence\"")
        || refusal_step.contains("effect(\"Publish\"")
        || refusal_step.contains("retainedCandidate")
        || refusal_step.contains("published:")
        || refusal_step.contains("lastSuccess:")
    {
        return Err(
            "HTML attempt admission refusal does not clear/start exactly without report publication"
                .to_owned(),
        );
    }
    let pending_step = trace_steps
        .iter()
        .find(|step| step.contains("id: \"admission-refusal-queue-r2\""))
        .ok_or("HTML lacks pending revision before admission refusal")?;
    if !pending_step.contains("pending: pendingRevision(\"r2\", \"e2\"") {
        return Err("HTML admission refusal does not start eligible pending work".to_owned());
    }
    validate_html_delivery_snapshots(executable, &trace_steps, summary)?;
    Ok(())
}

fn validate_html_delivery_snapshots(
    executable: &str,
    trace_steps: &[String],
    summary: &mut Summary,
) -> CheckResult<()> {
    for fragment in [
        "const idleDelivery = () => ({ adapter_state: \"Idle\", watch_delivery: null });",
        "session_instance: sessionInstance",
        "revision,",
        "evaluation: sessionEvaluationIdentity(sessionInstance, revision, evaluationOrdinal)",
        "publication_sequence: sessionPublicationIdentity(sessionInstance, publicationSequence)",
        "result_identity: resultIdentity",
        "outcome: compilationDeliveryOutcome(outcomeReference, compilationIdentity, resultIdentity, receiptStatus)",
        "compilation_identity: compilationIdentity",
        "result_identity: resultIdentity",
        "{ kind: \"compilation\", value: compilationIdentity }",
        "{ kind: \"result\", value: resultIdentity }",
        "kind: \"compilation_delivery\"",
        "compilation: compilationIdentity",
        "result: resultIdentity",
    ] {
        if !executable.contains(fragment) {
            return Err(format!(
                "HTML delivery helper lacks coherent fence fragment `{fragment}`"
            ));
        }
    }

    let current = between(
        executable,
        "const STOP_CURRENT_DELIVERY = deepFreeze(watchDelivery({",
        "}));",
        "HTML current delivery wrapper",
    )?;
    let last = between(
        executable,
        "const STOP_LAST_SUCCESSFUL_DELIVERY = deepFreeze(watchDelivery({",
        "}));",
        "HTML last successful delivery wrapper",
    )?;
    assert_delivery_wrapper(
        current,
        &[
            "sessionInstance: \"session-stop-01\"",
            "revisionOrdinal: 1",
            "evaluationOrdinal: 1",
            "publicationSequence: 7",
            "compilationIdentity: COMPILATION_A",
            "resultIdentity: RESULT_A",
            "outcomeReference: \"compilation-delivery-outcome:session-stop-01:7\"",
            "receiptStatus: \"failed\"",
        ],
        "current delivery",
    )?;
    assert_delivery_wrapper(
        last,
        &[
            "sessionInstance: \"session-stop-01\"",
            "revisionOrdinal: 0",
            "evaluationOrdinal: 0",
            "publicationSequence: 6",
            "compilationIdentity: COMPILATION_A",
            "resultIdentity: RESULT_A",
            "outcomeReference: \"compilation-delivery-outcome:session-stop-01:6\"",
            "receiptStatus: \"committed\"",
        ],
        "last successful delivery",
    )?;
    if current == last {
        return Err(
            "HTML current and last successful delivery wrappers are not independent".to_owned(),
        );
    }

    let mut current_snapshots = 0;
    let mut last_snapshots = 0;
    for step in trace_steps {
        if step.contains("delivery:") {
            if !step.contains("delivery: STOP_CURRENT_DELIVERY") {
                return Err(
                    "HTML trace has a non-idle current delivery outside its complete wrapper"
                        .to_owned(),
                );
            }
            current_snapshots += 1;
        }
        if step.contains("lastSuccessfulDelivery:") {
            if !step.contains("lastSuccessfulDelivery: STOP_LAST_SUCCESSFUL_DELIVERY") {
                return Err(
                    "HTML trace has a Last Successful Delivery outside its complete wrapper"
                        .to_owned(),
                );
            }
            last_snapshots += 1;
        }
    }
    if current_snapshots == 0 || current_snapshots != last_snapshots {
        return Err(format!(
            "HTML delivery snapshot counts differ: current {current_snapshots}, last {last_snapshots}"
        ));
    }
    summary.delivery_wrappers = 2;
    Ok(())
}

fn assert_delivery_wrapper(block: &str, expected: &[&str], label: &str) -> CheckResult<()> {
    for fragment in expected {
        if !block.contains(fragment) {
            return Err(format!("HTML {label} lacks `{fragment}`"));
        }
    }
    Ok(())
}

fn assert_trace_step(step: &str, event: &str, effect: &str, label: &str) -> CheckResult<()> {
    let events = called_tokens(step, "event(\"")?;
    let effects = called_tokens(step, "effect(\"")?;
    if events != BTreeSet::from([event.to_owned()]) || !effects.contains(effect) {
        return Err(format!(
            "{label}: expected event `{event}` and effect `{effect}`, got events {events:?}, effects {effects:?}"
        ));
    }
    Ok(())
}

fn assert_observation_trace(
    steps: &[String],
    id: &str,
    event: &str,
    effect: &str,
    evidence: &[&str],
) -> CheckResult<()> {
    let step = steps
        .iter()
        .find(|step| step.contains(&format!("id: \"{id}\"")))
        .ok_or_else(|| format!("HTML is missing trace step `{id}`"))?;
    assert_trace_step(step, event, effect, id)?;
    for fragment in evidence {
        if !step.contains(fragment) {
            return Err(format!("HTML trace step `{id}` lacks `{fragment}`"));
        }
    }
    Ok(())
}

fn extract_js_calls(source: &str, marker: &str) -> CheckResult<Vec<String>> {
    let mut calls = Vec::new();
    let mut offset = 0;
    while let Some(relative) = source[offset..].find(marker) {
        let start = offset + relative + marker.len();
        let bytes = source.as_bytes();
        let mut depth = 1_i32;
        let mut quote = None;
        let mut escaped = false;
        let mut end = start;
        for (relative_index, byte) in bytes[start..].iter().copied().enumerate() {
            let index = start + relative_index;
            if let Some(delimiter) = quote {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == delimiter {
                    quote = None;
                }
                continue;
            }
            match byte {
                b'\'' | b'"' | b'`' => quote = Some(byte),
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = index;
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Err(format!("unterminated JavaScript call `{marker}`"));
        }
        calls.push(source[start..end].to_owned());
        offset = end + 1;
    }
    Ok(calls)
}

fn called_tokens(source: &str, prefix: &str) -> CheckResult<BTreeSet<String>> {
    let mut tokens = BTreeSet::new();
    let mut remainder = source;
    while let Some(index) = remainder.find(prefix) {
        remainder = &remainder[index + prefix.len()..];
        let end = remainder
            .find('"')
            .ok_or_else(|| format!("unterminated token after `{prefix}`"))?;
        tokens.insert(remainder[..end].to_owned());
        remainder = &remainder[end + 1..];
    }
    Ok(tokens)
}

fn validate_source_manifest(
    cases: &Value,
    schema: &Value,
    serializer_probe: &str,
    summary: &mut Summary,
) -> CheckResult<()> {
    let allowed = [
        "rust_accessor",
        "adapter_input",
        "adapter_derivation",
        "constant",
    ];
    let mut leaves = BTreeMap::new();
    let mut seen_classes = BTreeSet::new();
    for entry in array_at(cases, "/source_manifest")? {
        let leaf = string_field(entry, "leaf")?;
        let class = string_field(entry, "source_class")?;
        let source = string_field(entry, "source")?;
        if !allowed.contains(&class) {
            return Err(format!(
                "source manifest `{leaf}` has invalid class `{class}`"
            ));
        }
        if source.is_empty() {
            return Err(format!("source manifest `{leaf}` has an empty source"));
        }
        let marker = entry.get("probe_marker").and_then(Value::as_str);
        let schema_pointer = entry.get("schema_pointer").and_then(Value::as_str);
        match (class, marker, schema_pointer) {
            ("rust_accessor", Some(marker), None) => {
                if marker != leaf {
                    return Err(format!(
                        "source manifest `{leaf}` probe marker must equal its leaf"
                    ));
                }
                let needle = format!("final-source: {marker}");
                let count = serializer_probe.matches(&needle).count();
                if count != 1 {
                    return Err(format!(
                        "source manifest `{leaf}` needs exactly one compile-probe marker `{needle}`, got {count}"
                    ));
                }
            }
            ("rust_accessor", _, _) => {
                return Err(format!(
                    "Rust accessor source manifest `{leaf}` must use one probe marker"
                ));
            }
            (_, None, Some(pointer)) => {
                if schema.pointer(pointer).is_none() {
                    return Err(format!(
                        "source manifest `{leaf}` has unresolved schema pointer `{pointer}`"
                    ));
                }
            }
            (_, Some(_), Some(_)) => {
                return Err(format!(
                    "source manifest `{leaf}` must have exactly one evidence mechanism"
                ));
            }
            (_, Some(_), None) => {
                return Err(format!(
                    "non-accessor source manifest `{leaf}` must use a schema pointer"
                ));
            }
            (_, None, None) => {
                return Err(format!(
                    "source manifest `{leaf}` has no probe marker or schema pointer"
                ));
            }
        }
        if leaves.insert(leaf, class).is_some() {
            return Err(format!(
                "source manifest leaf `{leaf}` has more than one source"
            ));
        }
        seen_classes.insert(class);
    }
    if seen_classes != allowed.into_iter().collect() {
        return Err(format!(
            "source manifest does not exercise all source classes: {seen_classes:?}"
        ));
    }

    let expected_poisons = [
        (
            "offline-from-cache-hit",
            "compilation.operational_inventory.admission.admitted_network",
            "semantic cache hit",
            "rust_accessor",
        ),
        (
            "P-from-K",
            "compilation.operational_inventory.role_execution.capacity.P",
            "admitted ready-worker capacity K",
            "rust_accessor",
        ),
        (
            "W-from-host",
            "compilation.operational_inventory.role_execution.engine_width.admitted",
            "host parallelism",
            "rust_accessor",
        ),
        (
            "isolation-from-placement",
            "compilation.operational_inventory.attempt_control.admitted_interruption",
            "worker placement",
            "rust_accessor",
        ),
        (
            "complete-timing-without-lease",
            "compilation.operational_inventory.reporting.fine_engine_timing",
            "timing request without reached lease",
            "rust_accessor",
        ),
        (
            "profile-as-admission",
            "compilation.operational_inventory.resources.admitted",
            "resource profile ceiling",
            "rust_accessor",
        ),
        (
            "fabricated-receipt-stage",
            "transport.receipt.stage_ledger.stages",
            "terminal success",
            "rust_accessor",
        ),
        (
            "fabricated-receipt-commit",
            "transport.receipt.stage_ledger.actual_commit",
            "requested commit",
            "rust_accessor",
        ),
        (
            "archive-recipe-from-zip-method",
            "format.request.selected_archive_encoding_identity",
            "observed ZIP member method",
            "constant",
        ),
        (
            "request-rejection-with-attempt-token",
            "session.publication.request_rejected",
            "fabricated attempt token",
            "rust_accessor",
        ),
        (
            "ingestion-failure-with-attempt-token",
            "session.publication.ingestion_failure",
            "fabricated attempt token",
            "rust_accessor",
        ),
        (
            "refused-transport-with-admitted-reached-fields",
            "transport.receipt.refused",
            "requested values copied into admission or stage ledger",
            "rust_accessor",
        ),
        (
            "object-count-from-role-literal",
            "transport.receipt.stage_ledger.object_count",
            "adapter role literal",
            "rust_accessor",
        ),
        (
            "font-policy-from-profile",
            "creation.request.font_scan_policy",
            "adapter profile default",
            "rust_accessor",
        ),
        (
            "domain-from-worker-placement",
            "compilation.domain.not_selected",
            "worker placement",
            "rust_accessor",
        ),
        (
            "publication-reason-coercion",
            "publication.format.transport_refusal.reason",
            "Representation Admission Refusal reason",
            "rust_accessor",
        ),
        (
            "cleanup-outcome-from-requirement",
            "transport.cleanup_outcome",
            "requested cleanup requirement",
            "rust_accessor",
        ),
        (
            "creation-reached-from-admitted",
            "creation.resources.reached",
            "admitted creation limits",
            "rust_accessor",
        ),
        (
            "session-preparation-from-compilation-profile",
            "session.preparation.exact",
            "broader compilation operation limits",
            "rust_accessor",
        ),
    ];
    let mut poison_ids = BTreeSet::new();
    for poison in array_at(cases, "/poison_inference_cases")? {
        let id = string_field(poison, "id")?;
        let target = string_field(poison, "target_leaf")?;
        let attempted = string_field(poison, "attempted_source")?;
        if !leaves.contains_key(target) {
            return Err(format!(
                "poison `{id}` targets unmanifested leaf `{target}`"
            ));
        }
        if attempted.is_empty() {
            return Err(format!("poison `{id}` has no attempted inference source"));
        }
        let (_, expected_target, expected_attempted, expected_class) = expected_poisons
            .iter()
            .find(|(expected_id, _, _, _)| *expected_id == id)
            .ok_or_else(|| format!("unexpected poison inference case `{id}`"))?;
        if target != *expected_target || attempted != *expected_attempted {
            return Err(format!(
                "poison `{id}` differs: expected target/source ({expected_target:?}, {expected_attempted:?}), got ({target:?}, {attempted:?})"
            ));
        }
        if leaves.get(target) != Some(expected_class) {
            return Err(format!(
                "poison `{id}` target `{target}` must retain source class `{expected_class}`"
            ));
        }
        if !poison_ids.insert(id) {
            return Err(format!("duplicate poison inference id `{id}`"));
        }
    }
    let expected = expected_poisons
        .iter()
        .map(|(id, _, _, _)| *id)
        .collect::<BTreeSet<_>>();
    if poison_ids != expected {
        return Err(format!(
            "poison inference cases differ: expected {expected:?}, got {poison_ids:?}"
        ));
    }
    summary.source_leaves = leaves.len();
    summary.poison_cases = poison_ids.len();
    Ok(())
}

fn between<'a>(source: &'a str, start: &str, end: &str, label: &str) -> CheckResult<&'a str> {
    let (_, remainder) = source
        .split_once(start)
        .ok_or_else(|| format!("{label}: missing start marker"))?;
    let (value, _) = remainder
        .split_once(end)
        .ok_or_else(|| format!("{label}: missing end marker"))?;
    Ok(value)
}

fn assert_string_array(
    value: &Value,
    pointer: &str,
    expected: &[&str],
    label: &str,
) -> CheckResult<()> {
    let actual = strings_at(value, pointer)?;
    if actual != expected {
        return Err(format!(
            "{label} differ: expected {expected:?}, got {actual:?}"
        ));
    }
    Ok(())
}

fn object_at<'a>(value: &'a Value, pointer: &str) -> CheckResult<&'a Map<String, Value>> {
    value
        .pointer(pointer)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("expected object at `{pointer}`"))
}

fn array_at<'a>(value: &'a Value, pointer: &str) -> CheckResult<&'a [Value]> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected array at `{pointer}`"))
}

fn strings_at<'a>(value: &'a Value, pointer: &str) -> CheckResult<Vec<&'a str>> {
    array_at(value, pointer)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("expected string in array at `{pointer}`"))
        })
        .collect()
}

fn field<'a>(value: &'a Value, name: &str) -> CheckResult<&'a Value> {
    value
        .get(name)
        .ok_or_else(|| format!("missing fixture field `{name}`"))
}

fn string_field<'a>(value: &'a Value, name: &str) -> CheckResult<&'a str> {
    field(value, name)?
        .as_str()
        .ok_or_else(|| format!("fixture field `{name}` is not a string"))
}

fn strings_field<'a>(value: &'a Value, name: &str) -> CheckResult<Vec<&'a str>> {
    let pointer = format!("/{name}");
    strings_at(value, &pointer)
}

fn bool_field(value: &Value, name: &str) -> CheckResult<bool> {
    field(value, name)?
        .as_bool()
        .ok_or_else(|| format!("fixture field `{name}` is not a boolean"))
}

fn check_eq<T>(actual: T, expected: T, label: &str) -> CheckResult<()>
where
    T: PartialEq + fmt::Debug,
{
    if actual != expected {
        return Err(format!("{label}: expected {expected:?}, got {actual:?}"));
    }
    Ok(())
}
