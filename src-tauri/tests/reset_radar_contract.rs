use quotatide_core::RadarSourceErrorCode;
use quotatide_lib::reset_radar::decode_reset_radar;

const ACTIVE_NOW_MS: i64 = 1_753_706_400_000 + 31_536_000_000;
const AFTER_EXPIRY_MS: i64 = 1_753_796_800_000 + 31_536_000_000;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/radar")
            .join(name),
    )
    .expect("fixture")
}

#[test]
fn valid_prediction_and_latest_announcement_are_normalized() {
    let snapshot = decode_reset_radar(&fixture("valid.json"), ACTIVE_NOW_MS).expect("valid radar");
    let prediction = snapshot
        .active_observation(ACTIVE_NOW_MS)
        .expect("active prediction");

    assert_eq!(prediction.chance().basis_points(), 7_500);
    assert_eq!(prediction.source_id(), "2081899343091843463");
    assert_eq!(
        snapshot
            .latest_announcement()
            .expect("announcement")
            .source_id(),
        "2081423555256782904"
    );
}

#[test]
fn null_missing_and_expired_watch_are_successful_without_a_prediction() {
    for (name, now) in [
        ("null-watch.json", AFTER_EXPIRY_MS),
        ("missing-watch-field.json", AFTER_EXPIRY_MS),
        ("expired.json", AFTER_EXPIRY_MS),
    ] {
        let snapshot = decode_reset_radar(&fixture(name), now).expect("valid empty watch");
        assert!(snapshot.active_observation(now).is_none(), "{name}");
    }
}

#[test]
fn malformed_probability_time_or_source_link_is_a_contract_failure() {
    for name in [
        "out-of-range.json",
        "invalid-time.json",
        "unsafe-source.json",
    ] {
        let error = decode_reset_radar(&fixture(name), ACTIVE_NOW_MS).expect_err(name);
        assert_eq!(error.code(), RadarSourceErrorCode::ContractViolation);
    }
}
