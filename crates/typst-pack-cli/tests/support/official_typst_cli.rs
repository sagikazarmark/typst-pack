use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const OFFICIAL_TYPST_ENV: &str = "TYPST_PACK_OFFICIAL_TYPST";
const REQUIRE_OFFICIAL_TYPST_ENV: &str = "TYPST_PACK_REQUIRE_OFFICIAL_TYPST";

#[derive(Debug)]
pub struct OfficialTypstCli {
    executable: PathBuf,
}

impl OfficialTypstCli {
    pub fn from_environment() -> Option<Self> {
        match std::env::var_os(OFFICIAL_TYPST_ENV) {
            Some(executable) => Some(Self {
                executable: executable.into(),
            }),
            None if std::env::var_os(REQUIRE_OFFICIAL_TYPST_ENV).is_some() => {
                panic!("{OFFICIAL_TYPST_ENV} is required by the official CLI parity gate")
            }
            None => None,
        }
    }

    pub fn require_version(&self, expected: &str) {
        let output = self
            .command()
            .arg("--version")
            .output()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to execute official Typst oracle `{}`: {error}",
                    self.executable.display()
                )
            });
        assert!(
            output.status.success(),
            "official Typst oracle version command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let version = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            version.split_whitespace().take(2).collect::<Vec<_>>(),
            ["typst", expected],
            "{OFFICIAL_TYPST_ENV} must identify the exact embedded Typst release"
        );
    }

    pub fn compile<I, S>(&self, current_dir: &Path, arguments: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let arguments = std::iter::once(OsStr::new("compile").to_owned())
            .chain(
                arguments
                    .into_iter()
                    .map(|argument| argument.as_ref().to_owned()),
            )
            .collect::<Vec<_>>();
        self.run(current_dir, arguments, &[])
    }

    pub fn run<I, S>(
        &self,
        current_dir: &Path,
        arguments: I,
        environment: &[(&str, &str)],
    ) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command()
            .current_dir(current_dir)
            .args(arguments)
            .envs(environment.iter().copied())
            .output()
            .unwrap_or_else(|error| panic!("failed to execute official Typst oracle: {error}"))
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        for variable in [
            "SOURCE_DATE_EPOCH",
            "TYPST_CERT",
            "TYPST_FEATURES",
            "TYPST_FONT_PATHS",
            "TYPST_IGNORE_EMBEDDED_FONTS",
            "TYPST_IGNORE_SYSTEM_FONTS",
            "TYPST_PACKAGE_CACHE_PATH",
            "TYPST_PACKAGE_PATH",
            "TYPST_ROOT",
        ] {
            command.env_remove(variable);
        }
        command
    }
}
