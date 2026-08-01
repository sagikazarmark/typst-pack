#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DifferentialCategory {
    Compiler,
    Diagnostics,
    Environment,
    Fonts,
    Html,
    Packages,
    PackOverrides,
    Pdf,
    Png,
    SharedRequests,
    Svg,
    World,
}

impl DifferentialCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compiler => "compiler",
            Self::Diagnostics => "diagnostics",
            Self::Environment => "environment",
            Self::Fonts => "fonts",
            Self::Html => "html",
            Self::Packages => "packages",
            Self::PackOverrides => "pack-overrides",
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::SharedRequests => "shared-requests",
            Self::Svg => "svg",
            Self::World => "world",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DifferentialSuite {
    IndependentOracle,
    OfficialCli,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DifferentialCoverage {
    pub category: DifferentialCategory,
    pub suites: &'static [DifferentialSuite],
}

use DifferentialCategory as Category;
use DifferentialSuite as Suite;

pub const DIFFERENTIAL_COVERAGE: &[DifferentialCoverage] = &[
    DifferentialCoverage {
        category: Category::Compiler,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::Diagnostics,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::Environment,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::Fonts,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::Html,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::Packages,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::PackOverrides,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::Pdf,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::Png,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::SharedRequests,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::Svg,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
    DifferentialCoverage {
        category: Category::World,
        suites: &[Suite::IndependentOracle, Suite::OfficialCli],
    },
];
