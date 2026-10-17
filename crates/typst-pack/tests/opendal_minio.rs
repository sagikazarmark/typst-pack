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
use typst_pack::archive::{DecodeLimits, decode, encode};
use typst_pack::opendal::{
    Error, FontSource, PackageRead, PackageSource, WriteOutcome, WritePolicy, insert_package,
    read_archive, read_fonts, read_package, read_project, write_archive, write_extraction,
    write_package_archive,
};
use typst_pack::{Pack, ProjectSnapshot, ReadLimits, create, extract};

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

    eprintln!("MinIO scenario: conditional create");
    conditional_create_preserves_existing_bytes(&admin).await;
    eprintln!("MinIO scenario: project read");
    project_read_hands_off_exact_bytes(&admin).await;
    eprintln!("MinIO scenario: paginated recursive font read");
    paginated_font_read_reaches_the_second_page(&admin).await;
    eprintln!("MinIO scenario: package registry and cache");
    package_registry_read_validates_then_populates_cache(&admin).await;
    eprintln!("MinIO scenario: Pack Archive write and read");
    pack_archive_write_and_read_preserve_exact_bytes(&admin).await;
    eprintln!("MinIO scenario: authorization and partial write");
    authorization_and_partial_write_evidence(&admin, &partial).await;
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

    let archive = encode(
        &Pack::builder("main.typ")
            .file("main.typ", b"= Conflict".to_vec())
            .build()
            .unwrap(),
    );
    let error = write_archive(admin, path, &archive, WritePolicy::CreateOrVerify)
        .await
        .unwrap_err();
    assert!(matches!(error.error, Error::Conflict { .. }));
    assert_eq!(admin.read(path).await.unwrap().to_vec(), b"first");
}

async fn project_read_hands_off_exact_bytes(admin: &Operator) {
    admin
        .write("project/main.typ", b"= MinIO".to_vec())
        .await
        .unwrap();
    admin
        .write("project/.typkignore", b"ignored.typ".to_vec())
        .await
        .unwrap();
    admin
        .write("project/empty.txt", Vec::<u8>::new())
        .await
        .unwrap();
    admin
        .write("project-sibling/untouched", b"sibling".to_vec())
        .await
        .unwrap();

    let snapshot = read_project(admin, "project/", "main.typ", &ReadLimits::default())
        .await
        .unwrap();

    assert_eq!(snapshot.file("main.typ"), Some(b"= MinIO".as_slice()));
    assert_eq!(
        snapshot.file(".typkignore"),
        Some(b"ignored.typ".as_slice())
    );
    assert_eq!(snapshot.file("empty.txt"), Some(b"".as_slice()));
    assert_eq!(snapshot.files().count(), 3);
    assert_eq!(
        admin
            .read("project-sibling/untouched")
            .await
            .unwrap()
            .to_vec(),
        b"sibling"
    );
}

async fn paginated_font_read_reaches_the_second_page(admin: &Operator) {
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
    let sources = [FontSource {
        prefix: "fonts/paged/".into(),
        embedded: false,
    }];

    let catalog = read_fonts(admin, &sources, &ReadLimits::default())
        .await
        .unwrap();

    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].container.data(), font);
}

async fn package_registry_read_validates_then_populates_cache(admin: &Operator) {
    let archive = package_archive();
    admin
        .write(
            "packages/registry/preview/example-1.2.3.tar.gz",
            archive.clone(),
        )
        .await
        .unwrap();
    let spec = "@preview/example:1.2.3".parse().unwrap();
    let sources = [
        PackageSource::ArchiveCache {
            operator: admin.clone(),
            prefix: "packages/cache/".into(),
        },
        PackageSource::Registry {
            operator: admin.clone(),
            prefix: "packages/registry/".into(),
        },
    ];
    let read = read_package(&sources, &spec, &ReadLimits::default())
        .await
        .unwrap();
    assert_eq!(read, PackageRead::RegistryArchive(archive.clone()));
    let project = ProjectSnapshot::new("main.typ", [("main.typ", "")]).unwrap();
    let mut request = create::Request::new(project);
    insert_package(&mut request, &spec, read, false, ReadLimits::default()).unwrap();
    let outcome = write_package_archive(admin, "packages/cache/", &spec, &archive)
        .await
        .unwrap();
    assert_eq!(outcome, WriteOutcome::Created);

    let cached = read_package(&sources, &spec, &ReadLimits::default())
        .await
        .unwrap();
    assert_eq!(cached, PackageRead::CachedArchive(archive));
}

async fn pack_archive_write_and_read_preserve_exact_bytes(admin: &Operator) {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"= Archive".to_vec())
        .build()
        .unwrap();
    let archive = encode(&pack);
    let path = "archives/document.typk";
    for expected in [WriteOutcome::Created, WriteOutcome::AlreadyMatching] {
        let outcome = write_archive(admin, path, &archive, WritePolicy::CreateOrVerify)
            .await
            .unwrap();
        assert_eq!(outcome, expected);
    }

    let read = read_archive(admin, path, 512 * 1024 * 1024).await.unwrap();
    assert_eq!(read.as_slice(), archive.as_slice());
    let decoded = decode(&read, DecodeLimits::default()).unwrap();
    assert_eq!(decoded.identity(), pack.identity());
}

async fn authorization_and_partial_write_evidence(admin: &Operator, partial: &Operator) {
    admin
        .write("authorization/secret.typk", b"secret".to_vec())
        .await
        .unwrap();
    let error = read_archive(partial, "authorization/secret.typk", 1024)
        .await
        .unwrap_err();
    assert!(
        matches!(&error, Error::Storage { source, .. } if source.kind() == ErrorKind::PermissionDenied),
        "{error:?}"
    );

    admin
        .write("publication/partial/unrelated.bin", b"untouched".to_vec())
        .await
        .unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .file("assets/logo.bin", b"logo".to_vec())
        .build()
        .unwrap();
    let plan = extract::plan(&pack, extract::Selection::default()).unwrap();
    let error = write_extraction(
        partial,
        "publication/partial/",
        &plan,
        WritePolicy::Overwrite,
    )
    .await
    .unwrap_err();

    assert!(error.indeterminate);
    assert!(matches!(
        &error.error,
        Error::Storage { path, source, .. }
            if path == "publication/partial/main.typ" && source.kind() == ErrorKind::PermissionDenied
    ));
    assert_eq!(
        error.completed,
        [(
            "publication/partial/assets/logo.bin".to_owned(),
            WriteOutcome::Written
        )]
    );
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
