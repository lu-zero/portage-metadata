use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

/// Stability level for an architecture keyword.
///
/// See [PMS 7.3.3](https://projects.gentoo.org/pms/9/pms.html#keywords).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stability {
    /// The package is stable on this architecture (e.g. `amd64`).
    Stable,
    /// The package is testing/unstable on this architecture (e.g. `~amd64`).
    Testing,
    /// The package is disabled on this architecture (e.g. `-amd64`).
    Disabled,
    /// All architectures are disabled (`-*`).
    DisabledAll,
}

/// A single architecture keyword entry from the `KEYWORDS` variable.
///
/// Each keyword consists of an architecture name and a stability level.
///
/// See [PMS 7.3.3](https://projects.gentoo.org/pms/9/pms.html#keywords).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Keyword {
    /// Architecture name (e.g. `amd64`, `arm64`, `x86`).
    pub arch: String,
    /// Stability classification.
    pub stability: Stability,
}

impl Keyword {
    /// Parse a space-separated `KEYWORDS` line into a list of keywords.
    ///
    /// # Examples
    ///
    /// ```
    /// use portage_metadata::{Keyword, Stability};
    ///
    /// let kws = Keyword::parse_line("amd64 ~arm64 -x86 -*").unwrap();
    /// assert_eq!(kws.len(), 4);
    /// assert_eq!(kws[0].stability, Stability::Stable);
    /// assert_eq!(kws[1].stability, Stability::Testing);
    /// assert_eq!(kws[2].stability, Stability::Disabled);
    /// assert_eq!(kws[3].stability, Stability::DisabledAll);
    /// ```
    pub fn parse_line(input: &str) -> Result<Vec<Keyword>> {
        input
            .split_whitespace()
            .map(|token| token.parse())
            .collect()
    }
}

impl FromStr for Keyword {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(Error::InvalidKeyword("empty keyword".to_string()));
        }

        if s == "-*" {
            return Ok(Keyword {
                arch: "*".to_string(),
                stability: Stability::DisabledAll,
            });
        }

        if let Some(arch) = s.strip_prefix('~') {
            if arch.is_empty() {
                return Err(Error::InvalidKeyword(s.to_string()));
            }
            Ok(Keyword {
                arch: arch.to_string(),
                stability: Stability::Testing,
            })
        } else if let Some(arch) = s.strip_prefix('-') {
            if arch.is_empty() {
                return Err(Error::InvalidKeyword(s.to_string()));
            }
            Ok(Keyword {
                arch: arch.to_string(),
                stability: Stability::Disabled,
            })
        } else {
            Ok(Keyword {
                arch: s.to_string(),
                stability: Stability::Stable,
            })
        }
    }
}

impl fmt::Display for Keyword {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.stability {
            Stability::Stable => write!(f, "{}", self.arch),
            Stability::Testing => write!(f, "~{}", self.arch),
            Stability::Disabled => write!(f, "-{}", self.arch),
            Stability::DisabledAll => write!(f, "-*"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stable() {
        let kw: Keyword = "amd64".parse().unwrap();
        assert_eq!(kw.arch, "amd64");
        assert_eq!(kw.stability, Stability::Stable);
    }

    #[test]
    fn parse_testing() {
        let kw: Keyword = "~arm64".parse().unwrap();
        assert_eq!(kw.arch, "arm64");
        assert_eq!(kw.stability, Stability::Testing);
    }

    #[test]
    fn parse_disabled() {
        let kw: Keyword = "-x86".parse().unwrap();
        assert_eq!(kw.arch, "x86");
        assert_eq!(kw.stability, Stability::Disabled);
    }

    #[test]
    fn parse_disabled_all() {
        let kw: Keyword = "-*".parse().unwrap();
        assert_eq!(kw.arch, "*");
        assert_eq!(kw.stability, Stability::DisabledAll);
    }

    #[test]
    fn parse_line() {
        let kws = Keyword::parse_line("amd64 ~arm64 -x86 -*").unwrap();
        assert_eq!(kws.len(), 4);
        assert_eq!(kws[0].arch, "amd64");
        assert_eq!(kws[1].arch, "arm64");
        assert_eq!(kws[2].arch, "x86");
        assert_eq!(kws[3].stability, Stability::DisabledAll);
    }

    #[test]
    fn parse_empty_line() {
        let kws = Keyword::parse_line("").unwrap();
        assert!(kws.is_empty());
    }

    #[test]
    fn display_round_trip() {
        for s in ["amd64", "~arm64", "-x86", "-*"] {
            let kw: Keyword = s.parse().unwrap();
            assert_eq!(kw.to_string(), s);
        }
    }

    #[test]
    fn invalid_empty() {
        assert!("".parse::<Keyword>().is_err());
    }

    #[test]
    fn invalid_bare_tilde() {
        assert!("~".parse::<Keyword>().is_err());
    }

    #[test]
    fn invalid_bare_dash() {
        assert!("-".parse::<Keyword>().is_err());
    }
}
