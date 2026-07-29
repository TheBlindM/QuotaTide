//! Framework-independent `QuotaTide` domain core.

use serde::Serialize;
use ts_rs::TS;

/// Public, non-secret metadata exposed by the desktop shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct BuildInfo {
    pub product_name: String,
    pub version: String,
    pub author: String,
    pub identifier: String,
    pub stage: String,
}

/// Returns the public metadata for this `QuotaTide` build.
#[must_use]
pub fn build_info() -> BuildInfo {
    BuildInfo {
        product_name: "QuotaTide".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        author: "TheBlind".to_owned(),
        identifier: "dev.theblind.quotatide".to_owned(),
        stage: "skeleton".to_owned(),
    }
}
