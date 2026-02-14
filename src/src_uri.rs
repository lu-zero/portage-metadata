use std::fmt;

use winnow::ascii::multispace0;
use winnow::combinator::{alt, cut_err, delimited, dispatch, opt, peek, preceded, repeat};
use winnow::error::{ContextError, ErrMode, StrContext};
use winnow::prelude::*;
use winnow::token::{any, take_while};

use crate::error::{Error, Result};

/// A single entry in a `SRC_URI` expression.
///
/// `SRC_URI` specifies the source files needed to build a package. Entries
/// may be plain URIs, renamed URIs (EAPI 2+: `url -> filename`), or
/// USE-conditional groups. EAPI 8+ supports selective URI restrictions
/// with `fetch+` and `mirror+` prefixes.
///
/// See [PMS 7.3.2](https://projects.gentoo.org/pms/9/pms.html#srcuri)
/// and [PMS 8.2](https://projects.gentoo.org/pms/9/pms.html#dependency-specification-format).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SrcUriEntry {
    /// A plain URI. The filename is derived from the last path component.
    Uri {
        /// The download URL.
        url: String,
        /// The target filename (last path component of the URL).
        filename: String,
        /// URI restriction prefix (EAPI 8+): `None`, `Some("fetch")`, or `Some("mirror")`.
        restriction: Option<String>,
    },
    /// A renamed URI (EAPI 2+): `url -> target`.
    Renamed {
        /// The download URL.
        url: String,
        /// The local filename to save as.
        target: String,
        /// URI restriction prefix (EAPI 8+): `None`, `Some("fetch")`, or `Some("mirror")`.
        restriction: Option<String>,
    },
    /// `flag? ( entries... )` or `!flag? ( entries... )` conditional group.
    UseConditional {
        /// USE flag name.
        flag: String,
        /// `true` for `!flag?` (negated conditional).
        negated: bool,
        /// Entries guarded by this flag.
        entries: Vec<SrcUriEntry>,
    },
    /// A bare parenthesized group `( entries... )`.
    Group(Vec<SrcUriEntry>),
}

impl SrcUriEntry {
    /// Parse a `SRC_URI` expression string into a list of entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use portage_metadata::SrcUriEntry;
    ///
    /// let entries = SrcUriEntry::parse(
    ///     "https://example.com/foo-1.0.tar.gz ssl? ( https://example.com/ssl.patch )"
    /// ).unwrap();
    /// assert_eq!(entries.len(), 2);
    /// ```
    pub fn parse(input: &str) -> Result<Vec<SrcUriEntry>> {
        parse_src_uri_string()
            .parse(input)
            .map_err(|e| Error::InvalidSrcUri(format!("{e}")))
    }
}

/// Extract filename from a URL (last path component).
fn filename_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or(url)
        .to_string()
}

impl fmt::Display for SrcUriEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SrcUriEntry::Uri {
                url, restriction, ..
            } => {
                if let Some(prefix) = restriction {
                    write!(f, "{prefix}+")?;
                }
                write!(f, "{url}")
            }
            SrcUriEntry::Renamed {
                url,
                target,
                restriction,
            } => {
                if let Some(prefix) = restriction {
                    write!(f, "{prefix}+")?;
                }
                write!(f, "{url} -> {target}")
            }
            SrcUriEntry::UseConditional {
                flag,
                negated,
                entries,
            } => {
                if *negated {
                    write!(f, "!")?;
                }
                write!(f, "{flag}? ( ")?;
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{entry}")?;
                }
                write!(f, " )")
            }
            SrcUriEntry::Group(entries) => {
                write!(f, "( ")?;
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{entry}")?;
                }
                write!(f, " )")
            }
        }
    }
}

// Winnow parsers

fn is_uri_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            ':' | '/'
                | '.'
                | '-'
                | '_'
                | '~'
                | '$'
                | '&'
                | '\''
                | '*'
                | '+'
                | ','
                | ';'
                | '='
                | '%'
                | '@'
                | '#'
                | '?'
        )
}

fn is_filename_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+')
}

fn is_flag_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '+'
}

fn parse_uri<'s>() -> impl Parser<&'s str, String, ErrMode<ContextError>> {
    take_while(1.., is_uri_char).map(|s: &str| s.to_string())
}

fn parse_restriction_prefix<'s>() -> impl Parser<&'s str, Option<String>, ErrMode<ContextError>> {
    opt(alt((
        "fetch+".map(|_| "fetch".to_string()),
        "mirror+".map(|_| "mirror".to_string()),
    )))
}

fn parse_filename<'s>() -> impl Parser<&'s str, String, ErrMode<ContextError>> {
    take_while(1.., is_filename_char).map(|s: &str| s.to_string())
}

/// Parse a single URI, optionally followed by `-> filename`.
fn parse_uri_entry<'s>() -> impl Parser<&'s str, SrcUriEntry, ErrMode<ContextError>> {
    (
        parse_restriction_prefix(),
        parse_uri(),
        opt(preceded((multispace0, "->", multispace0), parse_filename())),
    )
        .map(|(restriction, url, rename)| {
            if let Some(target) = rename {
                SrcUriEntry::Renamed {
                    url,
                    target,
                    restriction,
                }
            } else {
                let filename = filename_from_url(&url);
                SrcUriEntry::Uri {
                    url,
                    filename,
                    restriction,
                }
            }
        })
}

/// Parse `[!]flag? ( entries... )`.
fn parse_use_conditional<'s>() -> impl Parser<&'s str, SrcUriEntry, ErrMode<ContextError>> {
    move |input: &mut &'s str| {
        let negated = opt('!').parse_next(input)?.is_some();
        let flag: String = take_while(1.., is_flag_char)
            .map(|s: &str| s.to_string())
            .parse_next(input)?;
        '?'.parse_next(input)?;
        multispace0.parse_next(input)?;
        let entries = cut_err(delimited('(', parse_src_uri_entries, (multispace0, ')')))
            .context(StrContext::Label("USE conditional group"))
            .parse_next(input)?;
        Ok(SrcUriEntry::UseConditional {
            flag,
            negated,
            entries,
        })
    }
}

/// Parse `( entries... )` — bare parenthesized group.
fn parse_group<'s>() -> impl Parser<&'s str, SrcUriEntry, ErrMode<ContextError>> {
    delimited(
        '(',
        parse_src_uri_entries,
        cut_err((multispace0, ')')).context(StrContext::Label("closing ')'")),
    )
    .map(SrcUriEntry::Group)
}

/// Parse a single SRC_URI entry.
fn parse_src_uri_entry(input: &mut &str) -> ModalResult<SrcUriEntry> {
    dispatch! {peek(any);
        '(' => parse_group(),
        _ => alt((
            parse_use_conditional(),
            parse_uri_entry(),
        )),
    }
    .parse_next(input)
}

/// Parse zero or more SRC_URI entries separated by whitespace.
fn parse_src_uri_entries(input: &mut &str) -> ModalResult<Vec<SrcUriEntry>> {
    repeat(0.., preceded(multispace0, parse_src_uri_entry)).parse_next(input)
}

/// Parse a complete SRC_URI string.
pub(crate) fn parse_src_uri_string<'s>(
) -> impl Parser<&'s str, Vec<SrcUriEntry>, ErrMode<ContextError>> {
    move |input: &mut &'s str| {
        let entries = parse_src_uri_entries(input)?;
        multispace0.parse_next(input)?;
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_uri() {
        let entries = SrcUriEntry::parse("https://example.com/foo-1.0.tar.gz").unwrap();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SrcUriEntry::Uri {
                url,
                filename,
                restriction,
            } => {
                assert_eq!(url, "https://example.com/foo-1.0.tar.gz");
                assert_eq!(filename, "foo-1.0.tar.gz");
                assert_eq!(restriction, &None);
            }
            _ => panic!("expected Uri"),
        }
    }

    #[test]
    fn parse_renamed_uri() {
        let entries =
            SrcUriEntry::parse("https://github.com/archive/v1.0.tar.gz -> foo-1.0.tar.gz").unwrap();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SrcUriEntry::Renamed {
                url,
                target,
                restriction,
            } => {
                assert_eq!(url, "https://github.com/archive/v1.0.tar.gz");
                assert_eq!(target, "foo-1.0.tar.gz");
                assert_eq!(restriction, &None);
            }
            _ => panic!("expected Renamed"),
        }
    }

    #[test]
    fn parse_fetch_restricted_uri() {
        let entries = SrcUriEntry::parse("fetch+https://example.com/foo-1.0.tar.gz").unwrap();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SrcUriEntry::Uri {
                url, restriction, ..
            } => {
                assert_eq!(url, "https://example.com/foo-1.0.tar.gz");
                assert_eq!(restriction, &Some("fetch".to_string()));
            }
            _ => panic!("expected Uri"),
        }
    }

    #[test]
    fn parse_mirror_restricted_uri() {
        let entries = SrcUriEntry::parse("mirror+https://example.com/foo-1.0.tar.gz").unwrap();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SrcUriEntry::Uri {
                url, restriction, ..
            } => {
                assert_eq!(url, "https://example.com/foo-1.0.tar.gz");
                assert_eq!(restriction, &Some("mirror".to_string()));
            }
            _ => panic!("expected Uri"),
        }
    }

    #[test]
    fn parse_restricted_renamed_uri() {
        let entries =
            SrcUriEntry::parse("fetch+https://github.com/archive/v1.0.tar.gz -> foo-1.0.tar.gz")
                .unwrap();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SrcUriEntry::Renamed {
                url,
                target,
                restriction,
            } => {
                assert_eq!(url, "https://github.com/archive/v1.0.tar.gz");
                assert_eq!(target, "foo-1.0.tar.gz");
                assert_eq!(restriction, &Some("fetch".to_string()));
            }
            _ => panic!("expected Renamed"),
        }
    }

    #[test]
    fn parse_use_conditional() {
        let entries = SrcUriEntry::parse("ssl? ( https://example.com/ssl.patch )").unwrap();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SrcUriEntry::UseConditional {
                flag,
                negated,
                entries,
            } => {
                assert_eq!(flag, "ssl");
                assert!(!negated);
                assert_eq!(entries.len(), 1);
            }
            _ => panic!("expected UseConditional"),
        }
    }

    #[test]
    fn parse_negated_conditional() {
        let entries = SrcUriEntry::parse("!doc? ( https://example.com/minimal.tar.gz )").unwrap();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SrcUriEntry::UseConditional { flag, negated, .. } => {
                assert_eq!(flag, "doc");
                assert!(negated);
            }
            _ => panic!("expected UseConditional"),
        }
    }

    #[test]
    fn parse_multiple_uris() {
        let entries =
            SrcUriEntry::parse("https://example.com/a.tar.gz https://example.com/b.tar.gz")
                .unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn parse_mixed() {
        let entries = SrcUriEntry::parse(
            "https://example.com/src.tar.gz ssl? ( https://example.com/ssl.patch )",
        )
        .unwrap();
        assert_eq!(entries.len(), 2);
        assert!(matches!(&entries[0], SrcUriEntry::Uri { .. }));
        assert!(matches!(&entries[1], SrcUriEntry::UseConditional { .. }));
    }

    #[test]
    fn parse_empty() {
        let entries = SrcUriEntry::parse("").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn display_uri() {
        let entry = SrcUriEntry::Uri {
            url: "https://example.com/foo.tar.gz".to_string(),
            filename: "foo.tar.gz".to_string(),
            restriction: None,
        };
        assert_eq!(entry.to_string(), "https://example.com/foo.tar.gz");
    }

    #[test]
    fn display_renamed() {
        let entry = SrcUriEntry::Renamed {
            url: "https://example.com/v1.tar.gz".to_string(),
            target: "foo-1.tar.gz".to_string(),
            restriction: None,
        };
        assert_eq!(
            entry.to_string(),
            "https://example.com/v1.tar.gz -> foo-1.tar.gz"
        );
    }

    #[test]
    fn real_world_src_uri() {
        let input = "https://github.com/llvm/llvm-project/archive/llvmorg-10.0.0-rc1.tar.gz";
        let entries = SrcUriEntry::parse(input).unwrap();
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            SrcUriEntry::Uri { filename, .. } => {
                assert_eq!(filename, "llvmorg-10.0.0-rc1.tar.gz");
            }
            _ => panic!("expected Uri"),
        }
    }

    #[test]
    fn display_restricted_uri() {
        let entries = SrcUriEntry::parse("fetch+https://example.com/foo.tar.gz").unwrap();
        let displayed = entries[0].to_string();
        assert_eq!(displayed, "fetch+https://example.com/foo.tar.gz");
    }

    #[test]
    fn display_restricted_renamed_uri() {
        let entries =
            SrcUriEntry::parse("mirror+https://example.com/foo.tar.gz -> bar.tar.gz").unwrap();
        let displayed = entries[0].to_string();
        assert_eq!(
            displayed,
            "mirror+https://example.com/foo.tar.gz -> bar.tar.gz"
        );
    }
}
