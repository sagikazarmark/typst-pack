//! The ordered Font Catalog Pack Creation selects faces from.
//!
//! The catalog itself is core: its tests run on a build with no crate feature
//! enabled, except the ones that need real font bytes, which Typst only ships
//! with the `embedded-fonts` feature. The filesystem section covers the
//! catalog the reference Pack Assembler composes.

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod font_bytes;

use typst_pack::{FontCatalog, FontContainer, FontContainerError, FontDisposition};

#[test]
fn an_empty_catalog_offers_no_faces() {
    let catalog = FontCatalog::new();

    assert!(catalog.entries().is_empty());
}

#[test]
fn a_container_that_holds_no_readable_face_is_rejected() {
    assert_eq!(
        FontContainer::new(b"not a font".to_vec()).unwrap_err(),
        FontContainerError::NoReadableFace
    );
}

#[test]
fn a_disposition_names_whether_container_bytes_travel_in_the_pack() {
    assert!(FontDisposition::Embedded.is_embedded());
    assert!(!FontDisposition::External.is_embedded());
}

#[cfg(feature = "embedded-fonts")]
mod containers {
    use typst::foundations::Dict;
    use typst_pack::{
        CompilationAccessKind, CompilationAccessOutcome, CompilationOutputSpecification,
        CompilationStatus, Digest, DiscoverySpecification, DocumentTime, FontCatalog,
        FontCatalogEntry, FontContainer, FontDisposition, PackCompilationRequest,
        PackCreationInput, PackCreationOutcome, PackOverrideSet, PackageCatalog,
        PackageReadFailures, ProjectSnapshotAssembly, SvgOutputSpecification, TypstTarget, compile,
        create, typst_embedded_font_containers,
    };

    use crate::font_bytes::{font_collection, typst_container};

    /// A two-face collection holding one container's face twice, so that
    /// container-local face order is observable.
    fn two_face_collection(font: &[u8]) -> Vec<u8> {
        font_collection(&[font.to_vec(), font.to_vec()])
    }

    #[test]
    fn standalone_container_digest_depends_only_on_exact_bytes() {
        let data = typst_container();
        let expected = Digest::of(&data);

        let first = FontContainer::new(data.clone()).unwrap();
        let second = FontContainer::new(data).unwrap();

        assert_eq!(first.digest(), expected);
        assert_eq!(second.digest(), expected);
        assert_eq!(first.faces().len(), 1);
        assert_eq!(first.faces()[0].index, 0);
    }

    #[test]
    fn faces_are_listed_in_container_local_index_order() {
        let collection = two_face_collection(&typst_container());
        let container = FontContainer::new(collection).unwrap();

        assert_eq!(
            container
                .faces()
                .iter()
                .map(|face| face.index)
                .collect::<Vec<_>>(),
            [0, 1]
        );
    }

    #[test]
    fn catalog_entries_keep_the_order_and_disposition_the_caller_chose() {
        let container = FontContainer::new(typst_container()).unwrap();
        let mut catalog = FontCatalog::from_iter([FontCatalogEntry::new(
            container.clone(),
            FontDisposition::External,
        )]);
        catalog.push(FontCatalogEntry::new(container, FontDisposition::Embedded));
        catalog.extend(
            typst_embedded_font_containers()
                .map(|container| FontCatalogEntry::new(container, FontDisposition::External)),
        );

        let entries = catalog.entries();
        assert!(entries.len() > 2);
        assert_eq!(entries[0].container(), entries[1].container());
        assert_eq!(entries[0].disposition(), FontDisposition::External);
        assert_eq!(entries[1].disposition(), FontDisposition::Embedded);
    }

    /// The exact bytes of the container Typst ships one family and weight in.
    fn typst_face(family: &str, weight: u16) -> Vec<u8> {
        typst_kit::fonts::embedded()
            .find(|(font, _)| {
                font.info().family == family
                    && font.info().variant.weight.to_number() == weight
                    && font.info().variant.style == typst::text::FontStyle::Normal
            })
            .map(|(font, _)| font.data().to_vec())
            .unwrap_or_else(|| panic!("Typst ships `{family}` at weight {weight}"))
    }

    /// Discovery that rendered only the regular face must not hide the other
    /// faces of the container from a later compile.
    #[test]
    fn a_compile_can_use_faces_discovery_did_not_render() {
        let collection = font_collection(&[
            typst_face("Libertinus Serif", 400),
            typst_face("Libertinus Serif", 700),
        ]);
        let project = ProjectSnapshotAssembly::new("main.typ")
            .assemble([(
                "main.typ",
                b"#set text(font: \"Libertinus Serif\")\nRegular".to_vec(),
            )])
            .unwrap();
        let fonts = FontCatalog::from_iter([FontCatalogEntry::new(
            FontContainer::new(collection.clone()).unwrap(),
            FontDisposition::Embedded,
        )]);
        let discovery =
            DiscoverySpecification::new(TypstTarget::Paged, Dict::new(), DocumentTime::Absent, [])
                .unwrap();
        let PackCreationOutcome::Created { pack, .. } = create(PackCreationInput {
            project: &project,
            packages: &PackageCatalog::new(),
            fonts: &fonts,
            package_failures: &PackageReadFailures::new(),
            discovery: &discovery,
            metadata: None,
        })
        .unwrap() else {
            panic!("creation needs no packages");
        };
        assert_eq!(pack.font_requirements().len(), 1);
        assert_eq!(pack.font_requirements()[0].faces.len(), 2);

        let overrides = PackOverrideSet::new(&pack)
            .replace(
                "main.typ",
                b"#set text(font: \"Libertinus Serif\")\n*Bold*".to_vec(),
            )
            .unwrap();
        let report = compile(
            PackCompilationRequest::new(
                pack,
                CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
            )
            .overrides(overrides),
        )
        .unwrap();
        let result = report.result().unwrap();

        assert_eq!(result.status(), CompilationStatus::Succeeded);
        let used = result
            .access_trace()
            .observations()
            .filter(|observation| observation.kind() == CompilationAccessKind::Font)
            .filter_map(|observation| match observation.outcome() {
                CompilationAccessOutcome::Read { digest, .. } => {
                    Some((*digest, observation.font_index()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(used, [(Digest::of(&collection), Some(1))]);
    }
}

/// The Font Catalog the filesystem Pack Assembler composes from
/// system fonts, Typst's embedded fonts, and scanned font directories.
#[cfg(all(feature = "fs", feature = "embedded-fonts"))]
mod filesystem {
    use std::fs;
    use std::path::{Path, PathBuf};

    use typst_pack::{
        Digest, FilesystemPackAssembler, FilesystemPackAssemblerConfig,
        FilesystemPackAssemblyRequest,
    };

    use crate::font_bytes::{family_of, renamed_family, typst_container};

    /// Writes `data` as the only font file of a fresh directory.
    fn font_directory(dir: &Path, name: &str, data: &[u8]) -> PathBuf {
        let directory = dir.join(name);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("font.ttf"), data).unwrap();
        directory
    }

    /// Writes a project whose text selects one exact family.
    fn project_using_family(dir: &Path, family: &str) -> PathBuf {
        let project = dir.join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            format!("#set text(font: \"{family}\")\nSelected"),
        )
        .unwrap();
        project
    }

    #[test]
    fn catalog_order_decides_which_container_offers_a_family() {
        let dir = tempfile::tempdir().unwrap();
        let first = typst_container();
        // Same family and variant, distinct bytes: only catalog order can
        // decide between them.
        let mut second = first.clone();
        second.push(0);
        let project = project_using_family(dir.path(), &family_of(&first));
        let first_directory = font_directory(dir.path(), "first", &first);
        let second_directory = font_directory(dir.path(), "second", &second);

        let selected = |directories: [&Path; 2]| {
            let mut config = FilesystemPackAssemblerConfig::new()
                .system_fonts(false)
                .typst_embedded_fonts(false);
            for directory in directories {
                config = config.font_path(directory);
            }
            let assembler = FilesystemPackAssembler::new(config);
            let report = assembler
                .assemble(FilesystemPackAssemblyRequest::new(
                    &project,
                    Path::new("main.typ"),
                ))
                .unwrap();
            let requirements = report.pack().font_requirements();
            assert_eq!(requirements.len(), 1);
            requirements[0].digest
        };

        assert_eq!(
            selected([&first_directory, &second_directory]),
            Digest::of(&first)
        );
        assert_eq!(
            selected([&second_directory, &first_directory]),
            Digest::of(&second)
        );
    }

    #[test]
    fn one_catalog_produces_a_pack_with_mixed_font_dispositions() {
        let dir = tempfile::tempdir().unwrap();
        let typst_font = typst_container();
        let typst_family = family_of(&typst_font);
        let scanned_family = format!("Z{}", &typst_family[1..]);
        let scanned_font = renamed_family(&typst_font, &typst_family, &scanned_family);
        let fonts = font_directory(dir.path(), "fonts", &scanned_font);
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            format!("Typst's own\n\n#text(font: \"{scanned_family}\")[Scanned]"),
        )
        .unwrap();

        let assembler = FilesystemPackAssembler::new(
            FilesystemPackAssemblerConfig::new()
                .system_fonts(false)
                .font_path(&fonts),
        );
        let report = assembler
            .assemble(
                FilesystemPackAssemblyRequest::new(&project, Path::new("main.typ"))
                    .embed_fonts(true),
            )
            .unwrap();

        let requirements = report.pack().font_requirements();
        let embedded = requirements
            .iter()
            .filter(|requirement| requirement.embedded)
            .collect::<Vec<_>>();
        let external = requirements
            .iter()
            .filter(|requirement| !requirement.embedded)
            .collect::<Vec<_>>();
        assert_eq!(embedded.len(), 1, "the scanned container is embedded");
        assert_eq!(embedded[0].digest, Digest::of(&scanned_font));
        assert_eq!(external.len(), 1, "Typst's own container stays external");
        assert_eq!(external[0].digest, Digest::of(&typst_font));

        // Font requirements keep the candidate catalog's relative order:
        // Typst's containers precede the scanned directory.
        assert_eq!(
            requirements
                .iter()
                .map(|requirement| requirement.digest)
                .collect::<Vec<_>>(),
            [Digest::of(&typst_font), Digest::of(&scanned_font),]
        );
    }

    #[test]
    fn font_embedding_does_not_infer_disposition_from_container_bytes() {
        let dir = tempfile::tempdir().unwrap();
        // A scanned container whose bytes are exactly one Typst ships. Its
        // disposition comes from the catalog position it was supplied at, not
        // from a comparison against Typst's own containers.
        let data = typst_container();
        let project = project_using_family(dir.path(), &family_of(&data));
        let fonts = font_directory(dir.path(), "fonts", &data);

        let assembler = FilesystemPackAssembler::new(
            FilesystemPackAssemblerConfig::new()
                .system_fonts(false)
                .typst_embedded_fonts(false)
                .font_path(&fonts),
        );
        let report = assembler
            .assemble(
                FilesystemPackAssemblyRequest::new(&project, Path::new("main.typ"))
                    .embed_fonts(true),
            )
            .unwrap();

        let requirements = report.pack().font_requirements();
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].digest, Digest::of(&data));
        assert!(requirements[0].embedded);
    }
}
