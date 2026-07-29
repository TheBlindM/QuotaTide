use chrono::NaiveDate;
use quotatide_core::{
    DailyLimitSnapshot, DailyPolicyStatus, PolicyDayFact, PolicyError, QuotaPolicy,
    ThresholdTransition,
};

#[test]
fn default_policy_is_the_confirmed_seven_day_template() {
    let policy = QuotaPolicy::default_for_timezone("Asia/Shanghai").expect("default policy");

    assert_eq!(
        policy.base_micropoints(),
        [
            16_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000, 10_000_000, 10_000_000
        ]
    );
    assert!(policy.carry_workdays_enabled());
    assert_eq!(policy.policy_timezone().name(), "Asia/Shanghai");
}

#[test]
fn confirmed_weekday_unused_quota_is_shared_by_later_workdays() {
    let policy = QuotaPolicy::default_for_timezone("Asia/Shanghai").expect("policy");
    let monday = NaiveDate::from_ymd_opt(2026, 7, 27).expect("monday");
    let tuesday = NaiveDate::from_ymd_opt(2026, 7, 28).expect("tuesday");
    let days = policy
        .project_days(
            &[
                PolicyDayFact::new(monday, Some(6_000_000), true),
                PolicyDayFact::new(tuesday, Some(14_000_000), false),
            ],
            tuesday,
            1,
        )
        .expect("projection");

    assert_eq!(days[0].status, DailyPolicyStatus::Finalized);
    assert_eq!(days[1].base_micropoints, 16_000_000);
    assert_eq!(days[1].carry_micropoints, 2_500_000);
    assert_eq!(days[1].limit_micropoints, 18_500_000);
    assert_eq!(days[1].status, DailyPolicyStatus::Normal);
}

#[test]
fn completed_days_keep_their_policy_snapshot_after_an_edit() {
    let active = QuotaPolicy::new(
        [
            20_000_000, 20_000_000, 20_000_000, 20_000_000, 0, 10_000_000, 10_000_000,
        ],
        true,
        "America/New_York",
    )
    .expect("active policy");
    let monday = NaiveDate::from_ymd_opt(2026, 7, 27).expect("monday");
    let tuesday = NaiveDate::from_ymd_opt(2026, 7, 28).expect("tuesday");
    let days = active
        .project_days(
            &[
                PolicyDayFact::with_snapshot(
                    monday,
                    Some(5_000_000),
                    DailyLimitSnapshot::new(7, "Asia/Shanghai", 16_000_000, 1_000_000)
                        .expect("snapshot"),
                ),
                PolicyDayFact::new(tuesday, None, false),
            ],
            tuesday,
            8,
        )
        .expect("projection");

    assert_eq!(days[0].policy_revision_id, 7);
    assert_eq!(days[0].base_micropoints, 16_000_000);
    assert_eq!(days[0].carry_micropoints, 1_000_000);
    assert_eq!(days[0].policy_timezone.name(), "Asia/Shanghai");
    assert_eq!(days[1].policy_revision_id, 8);
    assert_eq!(days[1].base_micropoints, 20_000_000);
    assert_eq!(days[1].carry_micropoints, 3_000_000);
    assert_eq!(days[1].policy_timezone.name(), "America/New_York");
}

#[test]
fn policy_rejects_negative_overfull_and_invalid_timezone_drafts() {
    assert_eq!(
        QuotaPolicy::new(
            [
                -1, 16_000_000, 16_000_000, 16_000_000, 16_000_000, 10_000_000, 10_000_000
            ],
            true,
            "Asia/Shanghai",
        ),
        Err(PolicyError::InvalidBaseUnits),
    );
    assert_eq!(
        QuotaPolicy::new([20_000_000; 7], true, "Asia/Shanghai"),
        Err(PolicyError::BaseTotalExceeded),
    );
    assert_eq!(
        QuotaPolicy::new([0; 7], false, "Mars/Olympus"),
        Err(PolicyError::InvalidTimezone),
    );
}

#[test]
fn threshold_notifications_only_fire_when_a_boundary_is_crossed() {
    assert_eq!(
        QuotaPolicy::threshold_transition(DailyPolicyStatus::Normal, DailyPolicyStatus::Warning),
        Some(ThresholdTransition::Warning),
    );
    assert_eq!(
        QuotaPolicy::threshold_transition(DailyPolicyStatus::Warning, DailyPolicyStatus::Warning),
        None,
    );
    assert_eq!(
        QuotaPolicy::threshold_transition(DailyPolicyStatus::Warning, DailyPolicyStatus::Exceeded),
        Some(ThresholdTransition::Exceeded),
    );
    assert_eq!(
        QuotaPolicy::threshold_transition(DailyPolicyStatus::Exceeded, DailyPolicyStatus::Exceeded),
        None,
    );
}

#[test]
fn projection_emits_a_threshold_transition_from_the_persisted_prior_status() {
    let policy = QuotaPolicy::default_for_timezone("Asia/Shanghai").expect("policy");
    let today = NaiveDate::from_ymd_opt(2026, 7, 29).expect("today");
    let projection = policy
        .project_days(
            &[PolicyDayFact::new(today, Some(13_000_000), false)
                .with_previous_status(DailyPolicyStatus::Normal)],
            today,
            1,
        )
        .expect("projection");

    assert_eq!(projection[0].status, DailyPolicyStatus::Warning);
    assert_eq!(
        projection[0].threshold_transition,
        Some(ThresholdTransition::Warning)
    );
}

#[test]
fn unknown_overuse_weekends_and_new_weeks_do_not_mint_extra_carry() {
    let policy = QuotaPolicy::default_for_timezone("Asia/Shanghai").expect("policy");
    let monday = NaiveDate::from_ymd_opt(2026, 7, 27).expect("monday");
    let days = policy
        .project_days(
            &[
                PolicyDayFact::new(monday, Some(6_000_000), true),
                PolicyDayFact::new(monday.succ_opt().expect("tuesday"), None, true),
                PolicyDayFact::new(
                    monday
                        .checked_add_days(chrono::Days::new(2))
                        .expect("wednesday"),
                    Some(30_000_000),
                    true,
                ),
                PolicyDayFact::new(
                    monday
                        .checked_add_days(chrono::Days::new(5))
                        .expect("saturday"),
                    Some(0),
                    true,
                ),
                PolicyDayFact::new(
                    monday
                        .checked_add_days(chrono::Days::new(7))
                        .expect("next monday"),
                    None,
                    false,
                ),
            ],
            monday
                .checked_add_days(chrono::Days::new(7))
                .expect("today"),
            1,
        )
        .expect("projection");

    assert_eq!(days[1].carry_micropoints, 2_500_000);
    assert_eq!(days[2].carry_micropoints, 2_500_000);
    assert_eq!(days[3].carry_micropoints, 0);
    assert_eq!(days[4].carry_micropoints, 0);
}
