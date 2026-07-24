#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DifferentialCategory {
    Diagnostics,
    Fonts,
    Html,
    Packages,
    PackOverrides,
    Pdf,
    Png,
    SharedRequests,
    Svg,
}

impl DifferentialCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Diagnostics => "diagnostics",
            Self::Fonts => "fonts",
            Self::Html => "html",
            Self::Packages => "packages",
            Self::PackOverrides => "pack-overrides",
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::SharedRequests => "shared-requests",
            Self::Svg => "svg",
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
        category: Category::Diagnostics,
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
        suites: &[Suite::IndependentOracle],
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
];
