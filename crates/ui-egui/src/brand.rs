//! The product name shown in the interface. A rebranded build sets these at compile time:
//! `CADCRAFT_BRAND_NAME` (window title, Start page, About), `CADCRAFT_BRAND_PUBLISHER` and
//! `CADCRAFT_BRAND_URL` (About). Unset, the build is plain CADCraft.

/// The product name.
pub const NAME: &str = match option_env!("CADCRAFT_BRAND_NAME") {
    Some(n) => n,
    None => "CADCraft",
};

/// Who publishes this build (empty for CADCraft itself).
pub const PUBLISHER: &str = match option_env!("CADCRAFT_BRAND_PUBLISHER") {
    Some(p) => p,
    None => "",
};

/// The publisher's web site (empty for none).
pub const URL: &str = match option_env!("CADCRAFT_BRAND_URL") {
    Some(u) => u,
    None => "",
};

/// Where a rebranded build's Help > Check for Updates goes (empty for nowhere).
pub const UPDATES_URL: &str = match option_env!("CADCRAFT_BRAND_UPDATES_URL") {
    Some(u) => u,
    None => "",
};

/// Whether this is a modified, rebranded build rather than CADCraft by the ArtCraft team.
pub const REBRANDED: bool = option_env!("CADCRAFT_BRAND_NAME").is_some();
