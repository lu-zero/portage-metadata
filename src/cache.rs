use portage_atom::{DepEntry, Slot};

use crate::eapi::Eapi;
use crate::error::{Error, Result};
use crate::iuse::IUse;
use crate::keyword::Keyword;
use crate::license::LicenseExpr;
use crate::metadata::EbuildMetadata;
use crate::phase::Phase;
use crate::required_use::RequiredUseExpr;
use crate::restrict::RestrictExpr;
use crate::src_uri::SrcUriEntry;

/// A parsed md5-cache entry.
///
/// Represents a single file from `metadata/md5-cache/<category>/<package>-<version>`.
/// Contains the full ebuild metadata plus cache-specific fields (`md5`, `eclasses`).
///
/// See [PMS 14.2](https://projects.gentoo.org/pms/9/pms.html#mddict-cache-file-format).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    /// The ebuild metadata.
    pub metadata: EbuildMetadata,

    /// MD5 checksum of the ebuild file (from `_md5_`).
    pub md5: Option<String>,

    /// Eclass inheritance list with checksums (from `_eclasses_`).
    ///
    /// Each tuple is `(eclass_name, checksum)`.
    pub eclasses: Vec<(String, String)>,
}

impl CacheEntry {
    /// Parse a md5-cache file's contents into a `CacheEntry`.
    ///
    /// The input is the full text of a cache file. Lines are `KEY=VALUE`
    /// pairs in arbitrary order. Empty values may be omitted entirely.
    ///
    /// # Examples
    ///
    /// ```
    /// use portage_metadata::CacheEntry;
    ///
    /// let input = "\
    /// EAPI=7
    /// DESCRIPTION=Example package
    /// SLOT=0
    /// DEFINED_PHASES=compile install
    /// KEYWORDS=~amd64
    /// ";
    /// let entry = CacheEntry::parse(input).unwrap();
    /// assert_eq!(entry.metadata.description, "Example package");
    /// ```
    pub fn parse(input: &str) -> Result<CacheEntry> {
        let mut eapi = None;
        let mut description = None;
        let mut slot = None;
        let mut homepage = String::new();
        let mut src_uri = String::new();
        let mut license = String::new();
        let mut keywords = String::new();
        let mut iuse = String::new();
        let mut required_use = String::new();
        let mut restrict = String::new();
        let mut properties = String::new();
        let mut depend = String::new();
        let mut rdepend = String::new();
        let mut bdepend = String::new();
        let mut pdepend = String::new();
        let mut idepend = String::new();
        let mut inherited = String::new();
        let mut defined_phases = String::new();
        let mut md5 = None;
        let mut eclasses_raw = String::new();

        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "EAPI" => eapi = Some(value.to_string()),
                    "DESCRIPTION" => description = Some(value.to_string()),
                    "SLOT" => slot = Some(value.to_string()),
                    "HOMEPAGE" => homepage = value.to_string(),
                    "SRC_URI" => src_uri = value.to_string(),
                    "LICENSE" => license = value.to_string(),
                    "KEYWORDS" => keywords = value.to_string(),
                    "IUSE" => iuse = value.to_string(),
                    "REQUIRED_USE" => required_use = value.to_string(),
                    "RESTRICT" => restrict = value.to_string(),
                    "PROPERTIES" => properties = value.to_string(),
                    "DEPEND" => depend = value.to_string(),
                    "RDEPEND" => rdepend = value.to_string(),
                    "BDEPEND" => bdepend = value.to_string(),
                    "PDEPEND" => pdepend = value.to_string(),
                    "IDEPEND" => idepend = value.to_string(),
                    "INHERITED" => inherited = value.to_string(),
                    "DEFINED_PHASES" => defined_phases = value.to_string(),
                    "_md5_" => md5 = Some(value.to_string()),
                    "_eclasses_" => eclasses_raw = value.to_string(),
                    _ => {} // Ignore unknown keys
                }
            }
        }

        let eapi_val = match eapi {
            Some(ref s) => s
                .parse::<Eapi>()
                .map_err(|_| Error::InvalidEapi(s.clone()))?,
            None => Eapi::Zero, // Default EAPI is 0
        };

        let description_val =
            description.ok_or_else(|| Error::MissingField("DESCRIPTION".to_string()))?;

        let slot_val = match slot {
            Some(ref s) => parse_slot(s)?,
            None => return Err(Error::MissingField("SLOT".to_string())),
        };

        let homepage_val: Vec<String> = if homepage.is_empty() {
            Vec::new()
        } else {
            homepage.split_whitespace().map(|s| s.to_string()).collect()
        };

        let src_uri_val = if src_uri.is_empty() {
            Vec::new()
        } else {
            SrcUriEntry::parse(&src_uri)?
        };

        let license_val = if license.is_empty() {
            None
        } else {
            Some(LicenseExpr::parse(&license)?)
        };

        let keywords_val = if keywords.is_empty() {
            Vec::new()
        } else {
            Keyword::parse_line(&keywords)?
        };

        let iuse_val = if iuse.is_empty() {
            Vec::new()
        } else {
            IUse::parse_line(&iuse)?
        };

        let required_use_val = if required_use.is_empty() {
            None
        } else {
            Some(RequiredUseExpr::parse(&required_use)?)
        };

        let restrict_val = if restrict.is_empty() {
            Vec::new()
        } else {
            RestrictExpr::parse(&restrict)?
        };

        let properties_val = if properties.is_empty() {
            Vec::new()
        } else {
            RestrictExpr::parse(&properties)?
        };

        let depend_val = parse_dep_field(&depend)?;
        let rdepend_val = parse_dep_field(&rdepend)?;
        let bdepend_val = parse_dep_field(&bdepend)?;
        let pdepend_val = parse_dep_field(&pdepend)?;
        let idepend_val = parse_dep_field(&idepend)?;

        let inherited_val: Vec<String> = if inherited.is_empty() {
            Vec::new()
        } else {
            inherited
                .split_whitespace()
                .map(|s| s.to_string())
                .collect()
        };

        let defined_phases_val = Phase::parse_line(&defined_phases)?;

        let eclasses = parse_eclasses(&eclasses_raw);

        Ok(CacheEntry {
            metadata: EbuildMetadata {
                eapi: eapi_val,
                description: description_val,
                slot: slot_val,
                homepage: homepage_val,
                src_uri: src_uri_val,
                license: license_val,
                keywords: keywords_val,
                iuse: iuse_val,
                required_use: required_use_val,
                restrict: restrict_val,
                properties: properties_val,
                depend: depend_val,
                rdepend: rdepend_val,
                bdepend: bdepend_val,
                pdepend: pdepend_val,
                idepend: idepend_val,
                inherited: inherited_val,
                defined_phases: defined_phases_val,
            },
            md5,
            eclasses,
        })
    }

    /// Serialize this cache entry back to md5-cache format.
    ///
    /// Produces a string suitable for writing to a cache file.
    /// Empty-valued fields are omitted.
    pub fn serialize(&self) -> String {
        let m = &self.metadata;
        let mut lines = Vec::new();

        // Always emit mandatory fields
        lines.push(format!(
            "DEFINED_PHASES={}",
            format_phases(&m.defined_phases)
        ));

        if !m.depend.is_empty() {
            lines.push(format!("DEPEND={}", format_dep_entries(&m.depend)));
        }

        lines.push(format!("DESCRIPTION={}", m.description));
        lines.push(format!("EAPI={}", m.eapi));

        if !m.homepage.is_empty() {
            lines.push(format!("HOMEPAGE={}", m.homepage.join(" ")));
        }

        if !m.iuse.is_empty() {
            let iuse_str: Vec<String> = m.iuse.iter().map(|i| i.to_string()).collect();
            lines.push(format!("IUSE={}", iuse_str.join(" ")));
        }

        if !m.keywords.is_empty() {
            let kw_str: Vec<String> = m.keywords.iter().map(|k| k.to_string()).collect();
            lines.push(format!("KEYWORDS={}", kw_str.join(" ")));
        }

        if let Some(ref lic) = m.license {
            lines.push(format!("LICENSE={}", lic));
        }

        if !m.pdepend.is_empty() {
            lines.push(format!("PDEPEND={}", format_dep_entries(&m.pdepend)));
        }

        if !m.rdepend.is_empty() {
            lines.push(format!("RDEPEND={}", format_dep_entries(&m.rdepend)));
        }

        if let Some(ref ru) = m.required_use {
            lines.push(format!("REQUIRED_USE={}", ru));
        }

        if !m.restrict.is_empty() {
            let r_str: Vec<String> = m.restrict.iter().map(|r| r.to_string()).collect();
            lines.push(format!("RESTRICT={}", r_str.join(" ")));
        }

        lines.push(format!("SLOT={}", m.slot));

        if !m.src_uri.is_empty() {
            let uri_str: Vec<String> = m.src_uri.iter().map(|u| u.to_string()).collect();
            lines.push(format!("SRC_URI={}", uri_str.join(" ")));
        }

        if !m.bdepend.is_empty() {
            lines.push(format!("BDEPEND={}", format_dep_entries(&m.bdepend)));
        }

        if !m.idepend.is_empty() {
            lines.push(format!("IDEPEND={}", format_dep_entries(&m.idepend)));
        }

        if !m.properties.is_empty() {
            let p_str: Vec<String> = m.properties.iter().map(|p| p.to_string()).collect();
            lines.push(format!("PROPERTIES={}", p_str.join(" ")));
        }

        if !m.inherited.is_empty() {
            lines.push(format!("INHERITED={}", m.inherited.join(" ")));
        }

        if !self.eclasses.is_empty() {
            let parts: Vec<String> = self
                .eclasses
                .iter()
                .flat_map(|(name, checksum)| vec![name.clone(), checksum.clone()])
                .collect();
            lines.push(format!("_eclasses_={}", parts.join("\t")));
        }

        if let Some(ref md5) = self.md5 {
            lines.push(format!("_md5_={}", md5));
        }

        lines.push(String::new()); // trailing newline
        lines.join("\n")
    }
}

/// Parse a SLOT value into a `Slot`.
fn parse_slot(s: &str) -> Result<Slot> {
    if s.is_empty() {
        return Err(Error::MissingField("SLOT".to_string()));
    }
    if let Some((slot, subslot)) = s.split_once('/') {
        Ok(Slot::with_subslot(slot, subslot))
    } else {
        Ok(Slot::new(s))
    }
}

/// Parse a dependency field value into `Vec<DepEntry>`.
fn parse_dep_field(s: &str) -> Result<Vec<DepEntry>> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    DepEntry::parse(s).map_err(|e| Error::DepError(format!("{e}")))
}

/// Parse the `_eclasses_` value: tab-separated pairs of `name\tchecksum`.
fn parse_eclasses(s: &str) -> Vec<(String, String)> {
    if s.is_empty() {
        return Vec::new();
    }
    let parts: Vec<&str> = s.split('\t').collect();
    parts
        .chunks(2)
        .filter_map(|chunk| {
            if chunk.len() == 2 {
                Some((chunk[0].to_string(), chunk[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

/// Format DEFINED_PHASES for serialization.
fn format_phases(phases: &[Phase]) -> String {
    if phases.is_empty() {
        "-".to_string()
    } else {
        let strs: Vec<String> = phases.iter().map(|p| p.to_string()).collect();
        strs.join(" ")
    }
}

/// Format dependency entries for serialization.
fn format_dep_entries(entries: &[DepEntry]) -> String {
    let strs: Vec<String> = entries.iter().map(|e| e.to_string()).collect();
    strs.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eapi::Eapi;
    use crate::keyword::Stability;

    const EXAMPLE_CACHE: &str = "\
DEFINED_PHASES=install test unpack
DEPEND=>=sys-devel/clang-10.0.0_rc1:* dev-python/setuptools
DESCRIPTION=Python bindings for sys-devel/clang
EAPI=7
HOMEPAGE=https://llvm.org/
IUSE=test python_targets_python3_6 python_targets_python3_7
KEYWORDS=~amd64 ~x86
LICENSE=Apache-2.0-with-LLVM-exceptions UoI-NCSA
RDEPEND=>=sys-devel/clang-10.0.0_rc1:*
REQUIRED_USE=|| ( python_targets_python3_6 python_targets_python3_7 )
RESTRICT=!test? ( test )
SLOT=0
SRC_URI=https://github.com/llvm/llvm-project/archive/llvmorg-10.0.0-rc1.tar.gz
_eclasses_=llvm.org\t4e92abc\tmultibuild\t40fe1234
_md5_=4539d849d3cea8ac84debad9b3154143
";

    #[test]
    fn parse_example() {
        let entry = CacheEntry::parse(EXAMPLE_CACHE).unwrap();
        assert_eq!(entry.metadata.eapi, Eapi::Seven);
        assert_eq!(
            entry.metadata.description,
            "Python bindings for sys-devel/clang"
        );
        assert_eq!(entry.metadata.slot.slot, "0");
        assert_eq!(entry.metadata.slot.subslot, None);
        assert_eq!(entry.metadata.homepage, vec!["https://llvm.org/"]);
        assert_eq!(entry.metadata.keywords.len(), 2);
        assert_eq!(entry.metadata.keywords[0].arch, "amd64");
        assert_eq!(entry.metadata.keywords[0].stability, Stability::Testing);
        assert_eq!(entry.metadata.iuse.len(), 3);
        assert!(entry.metadata.required_use.is_some());
        assert!(!entry.metadata.restrict.is_empty());
        assert_eq!(entry.metadata.defined_phases.len(), 3);
        assert_eq!(entry.metadata.src_uri.len(), 1);
        assert!(!entry.metadata.depend.is_empty());
        assert!(!entry.metadata.rdepend.is_empty());
        assert!(entry.metadata.bdepend.is_empty()); // EAPI 7 but no BDEPEND in this example
        assert_eq!(
            entry.md5,
            Some("4539d849d3cea8ac84debad9b3154143".to_string())
        );
        assert_eq!(entry.eclasses.len(), 2);
        assert_eq!(entry.eclasses[0].0, "llvm.org");
        assert_eq!(entry.eclasses[1].0, "multibuild");
    }

    #[test]
    fn parse_minimal() {
        let input = "DESCRIPTION=Minimal\nSLOT=0\n";
        let entry = CacheEntry::parse(input).unwrap();
        assert_eq!(entry.metadata.eapi, Eapi::Zero);
        assert_eq!(entry.metadata.description, "Minimal");
        assert_eq!(entry.metadata.slot.slot, "0");
    }

    #[test]
    fn missing_description() {
        let input = "EAPI=7\nSLOT=0\n";
        let err = CacheEntry::parse(input).unwrap_err();
        assert!(matches!(err, Error::MissingField(ref f) if f == "DESCRIPTION"));
    }

    #[test]
    fn missing_slot() {
        let input = "EAPI=7\nDESCRIPTION=Test\n";
        let err = CacheEntry::parse(input).unwrap_err();
        assert!(matches!(err, Error::MissingField(ref f) if f == "SLOT"));
    }

    #[test]
    fn slot_with_subslot() {
        let input = "DESCRIPTION=Test\nSLOT=0/2.1\n";
        let entry = CacheEntry::parse(input).unwrap();
        assert_eq!(entry.metadata.slot.slot, "0");
        assert_eq!(entry.metadata.slot.subslot, Some("2.1".to_string()));
    }

    #[test]
    fn parse_eclasses() {
        let eclasses = super::parse_eclasses("llvm.org\tabc123\tmultibuild\tdef456");
        assert_eq!(eclasses.len(), 2);
        assert_eq!(eclasses[0], ("llvm.org".to_string(), "abc123".to_string()));
        assert_eq!(
            eclasses[1],
            ("multibuild".to_string(), "def456".to_string())
        );
    }

    #[test]
    fn parse_eclasses_empty() {
        let eclasses = super::parse_eclasses("");
        assert!(eclasses.is_empty());
    }

    #[test]
    fn parse_eclasses_odd_count() {
        // Odd number of tab-separated values: last one is ignored
        let eclasses = super::parse_eclasses("llvm.org\tabc123\torphan");
        assert_eq!(eclasses.len(), 1);
    }

    #[test]
    fn serialize_round_trip() {
        let entry = CacheEntry::parse(EXAMPLE_CACHE).unwrap();
        let serialized = entry.serialize();
        let reparsed = CacheEntry::parse(&serialized).unwrap();
        assert_eq!(entry.metadata.eapi, reparsed.metadata.eapi);
        assert_eq!(entry.metadata.description, reparsed.metadata.description);
        assert_eq!(entry.metadata.slot, reparsed.metadata.slot);
        assert_eq!(
            entry.metadata.keywords.len(),
            reparsed.metadata.keywords.len()
        );
        assert_eq!(entry.md5, reparsed.md5);
        assert_eq!(entry.eclasses, reparsed.eclasses);
    }

    #[test]
    fn defined_phases_dash() {
        let input = "DESCRIPTION=Test\nSLOT=0\nDEFINED_PHASES=-\n";
        let entry = CacheEntry::parse(input).unwrap();
        assert!(entry.metadata.defined_phases.is_empty());
    }

    #[test]
    fn unknown_keys_ignored() {
        let input = "DESCRIPTION=Test\nSLOT=0\nFOO=bar\n";
        let entry = CacheEntry::parse(input).unwrap();
        assert_eq!(entry.metadata.description, "Test");
    }

    #[test]
    fn empty_lines_ignored() {
        let input = "\nDESCRIPTION=Test\n\nSLOT=0\n\n";
        let entry = CacheEntry::parse(input).unwrap();
        assert_eq!(entry.metadata.description, "Test");
    }

    #[test]
    fn license_parsing() {
        let input = "DESCRIPTION=Test\nSLOT=0\nLICENSE=Apache-2.0-with-LLVM-exceptions UoI-NCSA\n";
        let entry = CacheEntry::parse(input).unwrap();
        assert!(entry.metadata.license.is_some());
    }

    #[test]
    fn eapi8_idepend() {
        let input = "EAPI=8\nDESCRIPTION=Test\nSLOT=0\nIDEPEND=sys-apps/systemd\n";
        let entry = CacheEntry::parse(input).unwrap();
        assert_eq!(entry.metadata.eapi, Eapi::Eight);
        assert_eq!(entry.metadata.idepend.len(), 1);
    }
}
