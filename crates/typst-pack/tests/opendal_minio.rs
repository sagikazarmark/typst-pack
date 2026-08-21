#![cfg(all(
    target_os = "linux",
    feature = "opendal",
    feature = "package-reading",
    feature = "embedded-fonts",
))]

#[path = "support/fonts.rs"]
mod fonts;

use std::env;
use std::io::Write as _;

use futures_util::StreamExt as _;
use opendal::{ErrorKind, Operator};
use typst_pack::opendal::pack_archive::{
    PackArchiveReadErrorCause, PackArchiveReadRequest, read_pack_archive,
};
use typst_pack::opendal::pack_assembly::{
    FontReadLimits, FontReadRequest, FontSource, PackageRead, PackageReadLimits,
    PackageReadRequest, ProjectReadEntry, ProjectReadLimits, ProjectReadRequest,
    insert_read_package, read_fonts, read_package, read_project,
};
use typst_pack::opendal::write::{
    OpenDalWritePhase, PackArchiveWriteRequest, PackExtractionWriteErrorCause,
    PackExtractionWriteProgress, PackExtractionWriteRequest, PackageCacheArchiveWriteRequest,
    WritePolicy, write_pack_archive, write_pack_extraction_plan, write_package_cache_archive,
};
use typst_pack::opendal::{Location, OperatorBinding, OperatorBindings};
use typst_pack::pack_archive::{DecodeLimits, ReadLimits, decode, encode};
use typst_pack::{
    FontCatalog, FontCatalogEntry, FontContainer, FontDisposition, Pack, PackExtractionSelection,
    PackageCatalog, PackageDisposition, PackageExpansionLimits, PackageReadFailures,
    ProjectSnapshotAssembly, WriteKeyOutcome, plan_pack_extraction,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires the pinned MinIO Compose fixture"]
async fn minio_validates_surviving_storage_contracts() {
    opendal_http_transport_reqwest::install_default();
    let required = env::var("TYPST_PACK_REQUIRE_MINIO").as_deref() == Ok("1");
    let config = match MinioConfig::from_env() {
        Some(config) => config,
        None if !required => {
            eprintln!("MinIO test skipped: configuration is unavailable");
            return;
        }
        None => panic!("required MinIO configuration is unavailable"),
    };
    let admin = config.operator(&config.access_key, &config.secret_key);
    if admin.check().await.is_err() {
        if required {
            panic!("required MinIO endpoint is not ready");
        }
        eprintln!("MinIO test skipped: endpoint is unavailable");
        return;
    }
    let partial = config.operator(&config.partial_access_key, &config.partial_secret_key);
    let admin_bindings = bindings("minio", &admin);
    let partial_bindings = bindings("restricted", &partial);

    eprintln!("MinIO scenario: conditional create");
    conditional_create_preserves_existing_bytes(&admin).await;
    eprintln!("MinIO scenario: project read");
    project_read_hands_off_exact_bytes(&admin, &admin_bindings).await;
    eprintln!("MinIO scenario: paginated recursive font read");
    paginated_font_read_reaches_the_second_page(&admin, &admin_bindings).await;
    eprintln!("MinIO scenario: package registry and cache");
    package_registry_read_validates_then_populates_cache(&admin, &admin_bindings).await;
    eprintln!("MinIO scenario: Pack Archive write and read");
    pack_archive_write_and_read_preserve_exact_bytes(&admin_bindings).await;
    eprintln!("MinIO scenario: authorization and partial write");
    authorization_and_partial_write_evidence(&admin, &partial_bindings).await;
}

async fn conditional_create_preserves_existing_bytes(admin: &Operator) {
    let path = "contract/conditional-create";
    admin.write(path, b"first".to_vec()).await.unwrap();

    let error = admin
        .write_with(path, b"second".to_vec())
        .if_not_exists(true)
        .await
        .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::ConditionNotMatch);
    assert_eq!(admin.read(path).await.unwrap().to_vec(), b"first");
}

async fn project_read_hands_off_exact_bytes(admin: &Operator, bindings: &OperatorBindings) {
    admin
        .write("project/main.typ", b"= MinIO".to_vec())
        .await
        .unwrap();
    admin
        .write("project/.typkignore", b"ignored.typ".to_vec())
        .await
        .unwrap();
    admin
        .write("project-sibling/untouched", b"sibling".to_vec())
        .await
        .unwrap();
    let request = ProjectReadRequest::new(
        "minio:/project/".parse().unwrap(),
        ProjectReadLimits::reference_v1(),
    )
    .unwrap();

    let (_, entries) = read_project(bindings, &request).await.unwrap().into_parts();
    let snapshot = ProjectSnapshotAssembly::new("main.typ")
        .assemble(entries.into_iter().map(ProjectReadEntry::into_parts))
        .unwrap();

    assert_eq!(snapshot.file("main.typ"), Some(b"= MinIO".as_slice()));
    assert_eq!(
        snapshot.file(".typkignore"),
        Some(b"ignored.typ".as_slice())
    );
    assert_eq!(
        admin
            .read("project-sibling/untouched")
            .await
            .unwrap()
            .to_vec(),
        b"sibling"
    );
}

async fn paginated_font_read_reaches_the_second_page(
    admin: &Operator,
    bindings: &OperatorBindings,
) {
    futures_util::stream::iter(0..1_000)
        .for_each_concurrent(Some(32), |index| {
            let admin = admin.clone();
            async move {
                admin
                    .write(&format!("fonts/paged/{index:04}.txt"), b"x".to_vec())
                    .await
                    .unwrap();
            }
        })
        .await;
    let font = fonts::typst_container();
    admin
        .write("fonts/paged/nested/zz-container.TTF", font.clone())
        .await
        .unwrap();
    let request = FontReadRequest::new(
        [FontSource::new(
            "minio:/fonts/paged/".parse().unwrap(),
            FontDisposition::External,
        )],
        FontReadLimits::reference_v1(),
    )
    .unwrap();

    let (_, entries) = read_fonts(bindings, &request).await.unwrap().into_parts();
    assert_eq!(entries.len(), 1);
    let (_, _, path, disposition, bytes) = entries.into_iter().next().unwrap().into_parts();
    assert_eq!(path, "nested/zz-container.TTF");
    assert_eq!(bytes, font);
    let mut catalog = FontCatalog::new();
    catalog.push(FontCatalogEntry::new(
        FontContainer::new(bytes).unwrap(),
        disposition,
    ));
    assert_eq!(catalog.entries().len(), 1);
}

async fn package_registry_read_validates_then_populates_cache(
    admin: &Operator,
    bindings: &OperatorBindings,
) {
    let archive = package_archive();
    admin
        .write(
            "packages/registry/preview/example-1.2.3.tar.gz",
            archive.clone(),
        )
        .await
        .unwrap();
    let request = PackageReadRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [],
        Some("minio:/packages/cache/".parse().unwrap()),
        Some("minio:/packages/registry/".parse().unwrap()),
        PackageReadLimits::reference_v1(),
    )
    .unwrap();
    let read = read_package(bindings, &request).await.unwrap();
    assert!(matches!(read, PackageRead::RegistryArchive(_)));
    let mut catalog = PackageCatalog::new();
    let mut failures = PackageReadFailures::new();
    let residue = insert_read_package(
        &mut catalog,
        &mut failures,
        read,
        PackageDisposition::External,
        PackageExpansionLimits::reference_v1(),
    )
    .unwrap()
    .unwrap();
    let write = PackageCacheArchiveWriteRequest::new(residue.destination().clone()).unwrap();
    let receipt = write_package_cache_archive(bindings, &write, residue.bytes())
        .await
        .unwrap();
    assert_eq!(receipt.outcome(), WriteKeyOutcome::Created);

    let cached = read_package(bindings, &request).await.unwrap();
    let PackageRead::CachedArchive(cached) = cached else {
        panic!("expected the validated registry archive to become a cache hit");
    };
    assert_eq!(cached.bytes(), archive);
}

async fn pack_archive_write_and_read_preserve_exact_bytes(bindings: &OperatorBindings) {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"= Archive".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let archive = encode(&pack).unwrap();
    let destination: Location = "minio:/archives/document.typk".parse().unwrap();
    let write =
        PackArchiveWriteRequest::new(destination.clone(), WritePolicy::CreateOrVerify).unwrap();
    let receipt = write_pack_archive(bindings, &write, &archive)
        .await
        .unwrap();
    assert_eq!(receipt.outcome(), WriteKeyOutcome::Created);

    let read = PackArchiveReadRequest::new(destination, ReadLimits::reference_v1()).unwrap();
    let read = read_pack_archive(bindings, &read).await.unwrap();
    assert_eq!(read.as_slice(), archive.as_slice());
    let decoded = decode(&read, DecodeLimits::reference_v1()).unwrap();
    assert_eq!(decoded.identity(), pack.identity());
}

async fn authorization_and_partial_write_evidence(
    admin: &Operator,
    partial_bindings: &OperatorBindings,
) {
    admin
        .write("authorization/secret.typk", b"secret".to_vec())
        .await
        .unwrap();
    let read = PackArchiveReadRequest::new(
        "restricted:/authorization/secret.typk".parse().unwrap(),
        ReadLimits::reference_v1(),
    )
    .unwrap();
    let error = read_pack_archive(partial_bindings, &read)
        .await
        .unwrap_err();
    assert!(matches!(
        error.cause(),
        PackArchiveReadErrorCause::Read(source) if source.kind() == ErrorKind::PermissionDenied
    ));

    admin
        .write("publication/partial/unrelated.bin", b"untouched".to_vec())
        .await
        .unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file("assets/logo.bin", b"logo".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();
    let request = PackExtractionWriteRequest::new(
        "restricted:/publication/partial/".parse().unwrap(),
        WritePolicy::OverwriteExactKeys,
    )
    .unwrap();
    let mut progress = PackExtractionWriteProgress::new();
    let error = write_pack_extraction_plan(partial_bindings, &request, &plan, &mut progress)
        .await
        .unwrap_err();

    assert_eq!(error.phase(), OpenDalWritePhase::DirectWrite);
    assert_eq!(error.failed_relative_path(), Some("main.typ"));
    assert!(matches!(
        error.cause(),
        PackExtractionWriteErrorCause::DirectWrite(source)
            if source.kind() == ErrorKind::PermissionDenied
    ));
    assert_eq!(progress.completed().len(), 1);
    assert_eq!(progress.completed()[0].relative_path(), "assets/logo.bin");
    assert_eq!(progress.completed()[0].outcome(), WriteKeyOutcome::Written);
    assert_eq!(
        admin
            .read("publication/partial/assets/logo.bin")
            .await
            .unwrap()
            .to_vec(),
        b"logo"
    );
    assert!(!admin.exists("publication/partial/main.typ").await.unwrap());
    assert_eq!(
        admin
            .read("publication/partial/unrelated.bin")
            .await
            .unwrap()
            .to_vec(),
        b"untouched"
    );
}

fn bindings(name: &str, operator: &Operator) -> OperatorBindings {
    OperatorBindings::new([(OperatorBinding::new(name).unwrap(), operator.clone())]).unwrap()
}

fn package_archive() -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);
    for (path, bytes) in [
        (
            "typst.toml",
            b"[package]\nname = \"example\"\nversion = \"1.2.3\"\n".as_slice(),
        ),
        ("lib.typ", b"package library".as_slice()),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_path(path).unwrap();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append(&header, bytes).unwrap();
    }
    let mut encoder = archive.into_inner().unwrap();
    encoder.flush().unwrap();
    encoder.finish().unwrap()
}

struct MinioConfig {
    endpoint: String,
    bucket: String,
    prefix: String,
    access_key: String,
    secret_key: String,
    partial_access_key: String,
    partial_secret_key: String,
}

impl MinioConfig {
    fn from_env() -> Option<Self> {
        Some(Self {
            endpoint: env::var("TYPST_PACK_MINIO_ENDPOINT").ok()?,
            bucket: env::var("TYPST_PACK_MINIO_BUCKET").ok()?,
            prefix: env::var("TYPST_PACK_MINIO_PREFIX").ok()?,
            access_key: env::var("TYPST_PACK_MINIO_ACCESS_KEY").ok()?,
            secret_key: env::var("TYPST_PACK_MINIO_SECRET_KEY").ok()?,
            partial_access_key: env::var("TYPST_PACK_MINIO_PARTIAL_ACCESS_KEY").ok()?,
            partial_secret_key: env::var("TYPST_PACK_MINIO_PARTIAL_SECRET_KEY").ok()?,
        })
    }

    fn operator(&self, access_key: &str, secret_key: &str) -> Operator {
        let builder = opendal_service_s3::S3::default()
            .root(&self.prefix)
            .bucket(&self.bucket)
            .endpoint(&self.endpoint)
            .region("us-east-1")
            .access_key_id(access_key)
            .secret_access_key(secret_key)
            .disable_config_load()
            .disable_ec2_metadata();
        Operator::new(builder).unwrap()
    }
}
