use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use graphql_parser::schema::{Definition, Document, Field, Type, TypeDefinition, parse_schema};
use serde::Deserialize;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value, json};

type CheckResult<T> = Result<T, String>;

const SCHEMA_FILE: &str = "PROTOTYPE-first-party-cli-dagger-schemas.json";
const NATIVE_PROFILE_FILE: &str = "PROTOTYPE-native-cli-profile.json";
const DAGGER_PROFILE_FILE: &str = "PROTOTYPE-dagger-ci-profile.json";
const GRAPHQL_FILE: &str = "PROTOTYPE-first-party-cli-dagger-generated.graphql";
const HTML_FILE: &str = "PROTOTYPE-first-party-cli-dagger-contracts.html";
const SERIALIZER_PROBE_FILE: &str = "PROTOTYPE-first-party-cli-dagger-serializer-probe.rs";

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

fn main() {
    if let Err(error) = run() {
        eprintln!("issue-69 validation: FAILED: {error}");
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

    println!("issue-69 validation: ok");
    println!(
        "json-schema: Draft 2020-12, {} definitions, {} local refs, {} direct + {} generated cases",
        summary.definitions, summary.local_refs, summary.schema_cases, summary.generated_cases
    );
    println!("profiles: 2 valid; issue-69 limits and tightening relationships verified");
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
            ],
        ),
        (
            dagger,
            [
                ("/creation/package_files", "100000"),
                ("/creation/largest_package_file_bytes", "536870912"),
                ("/creation/font_candidates", "16384"),
                ("/execution/creation/isolated_worker_capacity", "2"),
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
            value["report"]["operational_inventory"]["role_execution"]["domain"]["width"] =
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
        "placement": "in_process",
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
        "capabilities": {"stability": "immutable", "race_closing_revalidation": false, "exact_key_revalidation": false, "opaque_scope_revalidation": false, "polling": false, "push_subscription": false, "cursor_replay": false, "network": "no_network"}
    })
}

fn authority_descriptor(role: &str) -> Value {
    let class_role = role.replace('_', "-");
    json!({
        "role": role,
        "descriptor_version": 1,
        "class": format!("org.typst-pack/native-cli/{class_role}/1"),
        "ordered_source_classes": ["caller_supplied"],
        "evidence": {"immutable_values": true, "exact_key_revalidation": false, "opaque_scope_revalidation": false, "polling": false, "push_subscription": false, "cursor_replay": false},
        "network": "no_network",
        "resolution_cache": "disabled",
        "private_caches": []
    })
}

fn creation_admission_refusal(native: &Value) -> Value {
    json!({
        "operation_request": creation_operation_request(),
        "requested_trust": "partially_trusted",
        "resource_profile": profile_reference(),
        "requested_limits": native["creation"].clone(),
        "evidence": evidence_descriptor(),
        "packages": authority_descriptor("package_authority"),
        "fonts": authority_descriptor("font_authority"),
        "execution": null,
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
            "enforcement": {"requested": [], "admitted": [], "reached": []}
        },
        "resources": {"profile": profile_reference(), "requested": native["creation"].clone(), "admitted": native["creation"].clone()},
        "dependency_execution": {
            "evidence": evidence_descriptor(), "packages": authority_descriptor("package_authority"), "fonts": authority_descriptor("font_authority"),
            "offline_roles_covered": ["creation_evidence", "package_authority", "font_authority"],
            "concurrency": {"symbol": "D", "requested": "1", "admitted": "1", "constraints": []}
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
            "timing": "not_requested", "fine_engine_timing": "not_requested", "fine_timing_lease_reached": false
        }
    })
}

fn managed_caller_thread_execution(width: &str) -> Value {
    json!({
        "kind": "caller_thread",
        "domain": {
            "kind": "managed", "identity": "org.typst-pack.engine-domain.prototype",
            "placement": "in_process", "width": width, "fine_timing_lease_reached": false
        },
        "engine_width": {
            "symbol": "W", "kind": "exact", "requested": {"kind": "automatic"},
            "admitted": width, "constraints": ["verified_available_capacity"]
        }
    })
}

fn managed_caller_thread_execution_exact(width: &str) -> Value {
    json!({
        "kind": "caller_thread",
        "domain": {
            "kind": "managed", "identity": "org.typst-pack.engine-domain.prototype",
            "placement": "in_process", "width": width, "fine_timing_lease_reached": false
        },
        "engine_width": {
            "symbol": "W", "kind": "exact", "requested": {"kind": "exact", "width": width},
            "admitted": width, "constraints": []
        }
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
        "representation" => native["representation"].clone(),
        "transport" => native["transport"].clone(),
        _ => unreachable!(),
    };
    json!({
        "trust": "partially_trusted", "network": "offline", "resource_profile": profile_reference(),
        "deadline": {"kind": "none"}, "cancellation_present": false, "interruption": "cooperative",
        "publication_strength": null, "cleanup_strength": null,
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
        "admission": {"kind": "refused", "requested": format_controls(native, "pack_ingress"), "reason": "unsupported_archive_encoding_recipe"}
    })
}

fn project_materialization_receipt(native: &Value) -> Value {
    let controls = format_controls(native, "representation");
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
                "counters": {"input_bytes": null, "output_bytes": "8", "control_record_bytes": null, "planned_objects": "1", "verified_objects": "1", "aggregate_decoded_bytes": "8", "file_count": "1"},
                "pack_exposed": true, "stable_value_completed": true, "timing": "not_requested",
                "publication": {"kind": "not_applicable"}, "cleanup": {"kind": "not_applicable"},
                "failure_class": "not_applicable", "failure_cause": null, "validation_rules": []
            },
            "file_count": "1", "aggregate_bytes": "8",
            "files": [{"path": "main.typ", "exact_bytes": "8", "content_identity": identity("exact-content", 'c')}]
        }
    })
}

fn archive_encoding_format_receipt(native: &Value) -> Value {
    let controls = format_controls(native, "representation");
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
                "counters": {"input_bytes": null, "output_bytes": "8", "control_record_bytes": "2", "planned_objects": "1", "verified_objects": "1", "aggregate_decoded_bytes": "8", "file_count": null},
                "pack_exposed": false, "stable_value_completed": true, "timing": "not_requested",
                "publication": {"kind": "not_applicable"}, "cleanup": {"kind": "not_applicable"},
                "failure_class": "not_applicable", "failure_cause": null, "validation_rules": []
            },
            "control_record_identity": identity("exact-content", 'a'),
            "output_archive_identity": identity("exact-content", 'b'),
            "closure_export_tree_identity": null
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
        "cleanup_requirements": ["complete_before_return"], "interruption": "cooperative",
        "enforcement": [], "timing_reporting": false
    })
}

fn transport_request(native: &Value) -> Value {
    json!({
        "requested_trust": "partially_trusted", "resource_profile": profile_reference(),
        "requested_limits": {"kind": "spool", "values": native["spool"].clone()},
        "requested_network": "offline", "covered_roles": ["spool"], "contractual_no_network": true,
        "requested_structural_network_enforcement": "not_claimed", "T": "1", "requested_commit": null,
        "requested_cleanup": "complete_before_return", "interruption": "cooperative", "cancellation_present": false,
        "monotonic_domain": "prototype-clock", "required_enforcement": [], "timing_requested": false,
        "deadline": {"kind": "none"}
    })
}

fn transport_refused(native: &Value) -> Value {
    json!({
        "schema": schema_header("org.typst-pack.transport-receipt"), "producer": producer(),
        "role": "spool", "status": "refused", "adapter_class": "cli",
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
        "requested_cleanup": "complete_before_return", "admitted_cleanup": "complete_before_return",
        "requested_interruption": "cooperative", "admitted_interruption": "cooperative", "cancellation_present": false,
        "monotonic_domain": "prototype-clock", "enforcement": {"requested": [], "admitted": [], "reached": []},
        "timing_requested": false, "timing_reporting_admitted": false, "deadline": {"kind": "none"},
        "descriptor": transport_descriptor()
    })
}

fn transport_stage_ledger() -> Value {
    json!({
        "stages": ["admission", "plan_freeze", "spooling", "transfer", "complete"], "primary_terminal": "complete",
        "object_count": "1", "transferred_bytes": "8", "actual_commit": null, "cleanup": "not_required",
        "residual_locator": null, "exposed_bytes": "8", "timing": {"status": "not_requested", "phases": []},
        "structural_network_enforcement_reached": "not_claimed", "enforcement_reached": [], "interruption_winner": "terminal_commitment"
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

fn compilation_operation_request() -> Value {
    json!({
        "network": "offline", "cache": {"kind": "disabled"}, "D": "1",
        "engine_width": {"kind": "automatic"}, "K": null, "Q": null, "P": null,
        "placement": "in_process", "interruption": "cooperative", "deadline": {"kind": "none"},
        "queue_timeout_ticks": null, "latency_target_ticks": null, "required_enforcement": [],
        "reporting": {"diagnostic_projection": false, "diagnostic_source_bundle": false, "timing": false, "fine_engine_timing": false}
    })
}

fn compilation_admission_refusal(native: &Value) -> Value {
    json!({
        "operation_request": compilation_operation_request(), "requested_trust": "partially_trusted",
        "resource_profile": profile_reference(), "requested_limits": native["compilation"].clone(),
        "packages": authority_descriptor("package_authority"), "fonts": authority_descriptor("font_authority"),
        "cache": null, "execution": null, "reason": "capacity"
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
            "structural_network_enforcement": "not_claimed", "enforcement": {"requested": [], "admitted": [], "reached": []}
        },
        "resources": {"profile": profile_reference(), "requested": native["compilation"].clone(), "admitted": native["compilation"].clone()},
        "dependency_execution": {
            "packages": authority_descriptor("package_authority"), "fonts": authority_descriptor("font_authority"),
            "cache_descriptor": null, "cache_policy": {"kind": "disabled"}, "cache_lookup": "disabled",
            "cache_isolation_domain_present": false, "offline_roles_covered": ["package_authority", "font_authority"],
            "concurrency": {"symbol": "D", "requested": "1", "admitted": "1", "constraints": []}
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
            "fine_engine_timing": "not_requested", "fine_timing_lease_reached": false
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
    json!({
        "schema": schema_header("org.typst-pack.compilation-report"), "producer": producer(),
        "request_inventory": accepted_request_inventory(), "operational_inventory": compilation_operational_inventory(native),
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

fn compile_operation_report_exact(native: &Value) -> Value {
    let mut value = compile_operation_report(native);
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
        "cleanup_requirements": ["complete_before_return"], "interruption": "cooperative",
        "enforcement": [], "timing_reporting": false
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
        "requested_cleanup": "complete_before_return", "admitted_cleanup": "complete_before_return",
        "requested_interruption": "cooperative", "admitted_interruption": "cooperative", "cancellation_present": false,
        "monotonic_domain": "prototype-clock", "enforcement": {"requested": [], "admitted": [], "reached": []},
        "timing_requested": false, "timing_reporting_admitted": false, "deadline": {"kind": "none"},
        "descriptor": delivery_transport_descriptor()
    })
}

fn committed_delivery_outcome(native: &Value) -> Value {
    let result = identity("compilation-result", 'd');
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
                    "primary_terminal": "complete", "object_count": "0", "transferred_bytes": "0",
                    "actual_commit": "complete_collection_atomic", "cleanup": "not_required", "residual_locator": null,
                    "exposed_bytes": "0", "timing": {"status": "not_requested", "phases": []},
                    "structural_network_enforcement_reached": "not_claimed", "enforcement_reached": [],
                    "interruption_winner": "terminal_commitment"
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
        "resource_profile": profile_reference(), "requested_limits": native["compilation"].clone(), "admitted_limits": native["compilation"].clone(),
        "request_inventory": [],
        "issues": [{"code": "unsupported_feature", "role": "feature", "declaration_ordinal": "0", "referenced_inventory_ordinal": null}]
    })
}

fn session_policy(native: &Value) -> Value {
    json!({
        "mode": "latest_only_complete_coverage",
        "preparation": {"resource_profile": profile_reference(), "requested_limits": native["compilation"].clone(), "admitted_limits": native["compilation"].clone()}
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
    matches!(
        lexical,
        Some("omitted_automatic" | "explicit_zero_automatic")
    ) && normalized == Some("automatic")
        && requested == Some("automatic")
        && admitted_jobs.is_some()
        && admitted_jobs == admitted_width
        && admitted_width == domain_width
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
    let increasing = stages.windows(2).all(|pair| rank[pair[0]] < rank[pair[1]])
        && stages.first().copied() == Some("admission")
        && stages.last().copied() == ledger.get("primary_terminal").and_then(Value::as_str);
    let commit_coherent = ledger["actual_commit"].is_null() || stages.contains(&"commit");
    let actual = increasing && commit_coherent;
    if actual != expected {
        return Err(format!(
            "{label}: expected semantic validity {expected}, got {actual}"
        ));
    }
    Ok(())
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
        (
            "TypstPackPackCreation",
            &["id", "status", "report", "pack", "requirePack"][..],
        ),
        (
            "TypstPackPackIngress",
            &["id", "status", "receipt", "pack", "requirePack"],
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
                "receipt",
                "stagingStatus",
                "stagingReceipt",
                "archive",
                "requireArchive",
            ],
        ),
        (
            "TypstPackClosureExport",
            &[
                "id",
                "status",
                "receipt",
                "stagingStatus",
                "stagingReceipt",
                "tree",
                "requireTree",
            ],
        ),
        (
            "TypstPackProjectMaterialization",
            &[
                "id",
                "status",
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
                let needle = format!("issue69-source: {marker}");
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
