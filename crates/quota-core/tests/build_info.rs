use quota_core::{BuildInfo, build_info};
use ts_rs::{Config, TS};

#[test]
fn build_info_exposes_only_public_product_metadata() {
    assert_eq!(
        build_info(),
        BuildInfo {
            product_name: "QuotaTide".to_owned(),
            version: "0.1.0".to_owned(),
            author: "TheBlind".to_owned(),
            identifier: "dev.theblind.quotatide".to_owned(),
            stage: "skeleton".to_owned(),
        }
    );
}

#[test]
fn build_info_typescript_contract_is_stable() {
    assert_eq!(
        BuildInfo::decl(&Config::default()),
        "type BuildInfo = { productName: string, version: string, author: string, identifier: string, stage: string, };"
    );
}
