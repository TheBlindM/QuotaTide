use quotatide_core::{
    AccountSettingsStore, RadarAnnouncement, RadarChance, RadarObservation, RadarSnapshot,
    RadarSourceErrorCode, SourceStatus, radar_bucket_label,
};
use tempfile::tempdir;

const NOW_MS: i64 = 1_785_283_200_000;

fn valid_observation(chance_basis_points: u16) -> RadarObservation {
    RadarObservation::new(
        "2081899343091843463",
        chance_basis_points,
        NOW_MS - 3_600_000,
        NOW_MS + 20 * 3_600_000,
        "Possible additional reset in the next 24 hours",
        "https://x.com/thsottiaux/status/2081899343091843463",
    )
    .expect("valid observation")
}

#[test]
fn probability_buckets_match_the_source_site_without_false_precision() {
    let cases = [
        (0, "<10%"),
        (999, "<10%"),
        (1_000, ">10%"),
        (6_999, ">60%"),
        (7_000, ">70%"),
        (7_599, ">70%"),
        (9_999, ">90%"),
        (10_000, ">90%"),
    ];

    for (basis_points, expected) in cases {
        let chance = RadarChance::from_basis_points(basis_points).expect("valid chance");
        assert_eq!(radar_bucket_label(chance), expected);
    }
    assert!(RadarChance::from_basis_points(10_001).is_none());
}

#[test]
fn an_expired_prediction_is_never_active_but_the_announcement_remains_visible() {
    let observation = valid_observation(7_500);
    let announcement = RadarAnnouncement::new(
        "2082317452755751098",
        NOW_MS - 86_400_000,
        "Global extra reset announced",
        "https://x.com/thsottiaux/status/2082317452755751098",
    )
    .expect("valid announcement");
    let snapshot = RadarSnapshot::new(Some(observation), Some(announcement.clone()));

    assert!(snapshot.active_observation(NOW_MS).is_some());
    assert!(
        snapshot
            .active_observation(NOW_MS + 20 * 3_600_000)
            .is_none()
    );
    assert_eq!(snapshot.latest_announcement(), Some(&announcement));
}

#[test]
fn invalid_time_order_or_unsafe_source_url_is_rejected() {
    assert!(
        RadarObservation::new(
            "2081899343091843463",
            7_500,
            NOW_MS,
            NOW_MS,
            "prediction",
            "https://x.com/thsottiaux/status/2081899343091843463",
        )
        .is_err()
    );
    assert!(
        RadarObservation::new(
            "2081899343091843463",
            7_500,
            NOW_MS,
            NOW_MS + 1,
            "prediction",
            "https://attacker.invalid/thsottiaux/status/2081899343091843463",
        )
        .is_err()
    );
}

#[tokio::test]
async fn radar_success_is_persisted_independently_and_empty_success_clears_current_watch() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let announcement = RadarAnnouncement::new(
        "2082317452755751098",
        NOW_MS - 86_400_000,
        "Global extra reset announced",
        "https://x.com/thsottiaux/status/2082317452755751098",
    )
    .expect("valid announcement");
    let disposition = store
        .record_radar_success(
            NOW_MS,
            RadarSnapshot::new(Some(valid_observation(7_500)), Some(announcement)),
        )
        .await
        .expect("record success");

    assert!(disposition.new_announcement);
    let active = store
        .public_reset_radar(NOW_MS)
        .await
        .expect("public radar");
    assert_eq!(active.source_status, SourceStatus::Fresh);
    assert_eq!(
        active.prediction.expect("prediction").display_chance,
        ">70%"
    );
    assert!(active.latest_announcement.is_some());

    store
        .record_radar_success(NOW_MS + 1, RadarSnapshot::new(None, None))
        .await
        .expect("record empty success");
    let empty = store
        .public_reset_radar(NOW_MS + 1)
        .await
        .expect("empty radar");
    assert!(empty.prediction.is_none());
    assert!(empty.latest_announcement.is_some());
    assert_eq!(empty.source_status, SourceStatus::Fresh);
}

#[tokio::test]
async fn a_failure_keeps_only_an_unexpired_last_known_good_prediction() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let observation = valid_observation(7_500);
    let expires_at = observation.expires_at_unix_ms();
    store
        .record_radar_success(NOW_MS, RadarSnapshot::new(Some(observation), None))
        .await
        .expect("record success");
    store
        .record_radar_failure(NOW_MS + 1, RadarSourceErrorCode::Timeout)
        .await
        .expect("record failure");

    let stale = store
        .public_reset_radar(NOW_MS + 1)
        .await
        .expect("stale radar");
    assert_eq!(stale.source_status, SourceStatus::StaleAfterFailure);
    assert_eq!(stale.public_error, Some(RadarSourceErrorCode::Timeout));
    assert!(stale.prediction.is_some());

    let expired = store
        .public_reset_radar(expires_at)
        .await
        .expect("expired radar");
    assert!(expired.prediction.is_none());
}
