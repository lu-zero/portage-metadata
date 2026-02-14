use portage_atom::{DepEntry, Slot};

use crate::eapi::Eapi;
use crate::iuse::IUse;
use crate::keyword::Keyword;
use crate::license::LicenseExpr;
use crate::phase::Phase;
use crate::required_use::RequiredUseExpr;
use crate::restrict::RestrictExpr;
use crate::src_uri::SrcUriEntry;

/// Metadata for a single ebuild, as produced by the metadata cache.
///
/// Contains all the PMS-defined metadata variables that a package manager
/// extracts from an ebuild. Mandatory fields (`eapi`, `description`, `slot`)
/// are always present; optional fields use `Option` or `Vec`.
///
/// See [PMS 7.2](https://projects.gentoo.org/pms/9/pms.html#mandatory-ebuilddefined-variables).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EbuildMetadata {
    /// EAPI version.
    ///
    /// See [PMS 7.3.1](https://projects.gentoo.org/pms/9/pms.html#eapi).
    pub eapi: Eapi,

    /// Package description (mandatory).
    ///
    /// See [PMS 7.2](https://projects.gentoo.org/pms/9/pms.html#mandatory-ebuilddefined-variables).
    pub description: String,

    /// Package slot (mandatory).
    ///
    /// See [PMS 7.2](https://projects.gentoo.org/pms/9/pms.html#mandatory-ebuilddefined-variables).
    pub slot: Slot,

    /// Homepage URL(s).
    pub homepage: Vec<String>,

    /// Source URI expression.
    pub src_uri: Vec<SrcUriEntry>,

    /// License expression.
    pub license: Option<LicenseExpr>,

    /// Architecture keywords.
    pub keywords: Vec<Keyword>,

    /// USE flags declared by the ebuild.
    pub iuse: Vec<IUse>,

    /// REQUIRED_USE expression (EAPI 4+).
    pub required_use: Option<RequiredUseExpr>,

    /// RESTRICT entries.
    pub restrict: Vec<RestrictExpr>,

    /// PROPERTIES entries.
    pub properties: Vec<RestrictExpr>,

    /// Build-time dependencies (`DEPEND`).
    ///
    /// See [PMS 8.1](https://projects.gentoo.org/pms/9/pms.html#dependency-classes).
    pub depend: Vec<DepEntry>,

    /// Runtime dependencies (`RDEPEND`).
    pub rdepend: Vec<DepEntry>,

    /// Build-host dependencies (`BDEPEND`, EAPI 7+).
    pub bdepend: Vec<DepEntry>,

    /// Post-merge dependencies (`PDEPEND`).
    pub pdepend: Vec<DepEntry>,

    /// Install-time dependencies (`IDEPEND`, EAPI 8).
    pub idepend: Vec<DepEntry>,

    /// Inherited eclasses.
    pub inherited: Vec<String>,

    /// Defined phase functions.
    pub defined_phases: Vec<Phase>,
}
