//! `QuotaTide` desktop shell.

use quota_core::BuildInfo;

/// Returns public metadata that proves the Rust core is connected to the UI.
#[tauri::command]
fn get_build_info() -> BuildInfo {
    quota_core::build_info()
}

/// Starts the `QuotaTide` desktop runtime.
///
/// # Panics
///
/// Panics when the desktop runtime cannot be initialized or its event loop fails.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_build_info])
        .run(tauri::generate_context!())
        .expect("failed to run the QuotaTide desktop shell");
}

#[cfg(test)]
mod tests {
    use super::get_build_info;

    #[test]
    fn command_returns_the_public_core_contract() {
        let info = get_build_info();

        assert_eq!(info.product_name, "QuotaTide");
        assert_eq!(info.identifier, "dev.theblind.quotatide");
        assert_eq!(info.stage, "skeleton");
    }
}
