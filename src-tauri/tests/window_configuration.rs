use std::{fs, path::Path};

use serde_json::Value;

#[test]
fn transparent_tray_window_does_not_use_the_rectangular_native_shadow() {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let config: Value =
        serde_json::from_str(&fs::read_to_string(config_path).expect("read Tauri configuration"))
            .expect("parse Tauri configuration");
    let main_window = config["app"]["windows"]
        .as_array()
        .and_then(|windows| {
            windows
                .iter()
                .find(|window| window["label"].as_str() == Some("main"))
        })
        .expect("main window configuration");

    assert_eq!(main_window["transparent"].as_bool(), Some(true));
    assert_eq!(main_window["decorations"].as_bool(), Some(false));
    assert_eq!(main_window["height"].as_u64(), Some(430));
    assert_eq!(main_window["minHeight"].as_u64(), Some(430));
    assert_eq!(main_window["maxHeight"].as_u64(), Some(602));
    assert_eq!(main_window["resizable"].as_bool(), Some(false));
    assert_eq!(
        main_window["shadow"].as_bool(),
        Some(false),
        "the OS shadow follows the rectangular native window, not the rounded web content",
    );
}
