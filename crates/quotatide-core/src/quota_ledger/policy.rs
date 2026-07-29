use std::collections::BTreeMap;

use chrono::{Datelike as _, NaiveDate};
use chrono_tz::Tz;

use super::QuotaLedger;

const DEFAULT_BASE_MICROPOINTS: [i64; 7] = [
    16_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000, 10_000_000, 10_000_000,
];

/// Validated seven-day quota policy used by the pure ledger projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaPolicy {
    base_micropoints: [i64; 7],
    carry_workdays_enabled: bool,
    policy_timezone: Tz,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDayFact {
    local_date: NaiveDate,
    used_micropoints: Option<i64>,
    finalized: bool,
    snapshot: Option<DailyLimitSnapshot>,
    previous_status: Option<DailyPolicyStatus>,
}

impl PolicyDayFact {
    #[must_use]
    pub const fn new(
        local_date: NaiveDate,
        used_micropoints: Option<i64>,
        finalized: bool,
    ) -> Self {
        Self {
            local_date,
            used_micropoints,
            finalized,
            snapshot: None,
            previous_status: None,
        }
    }

    #[must_use]
    pub const fn with_snapshot(
        local_date: NaiveDate,
        used_micropoints: Option<i64>,
        snapshot: DailyLimitSnapshot,
    ) -> Self {
        Self {
            local_date,
            used_micropoints,
            finalized: true,
            snapshot: Some(snapshot),
            previous_status: None,
        }
    }

    #[must_use]
    pub const fn with_previous_status(mut self, status: DailyPolicyStatus) -> Self {
        self.previous_status = Some(status);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyLimitSnapshot {
    policy_revision_id: u64,
    policy_timezone: Tz,
    base_micropoints: i64,
    carry_micropoints: i64,
}

impl DailyLimitSnapshot {
    /// Creates a completed day's immutable policy snapshot.
    ///
    /// # Errors
    ///
    /// Rejects negative units or an invalid IANA timezone.
    pub fn new(
        policy_revision_id: u64,
        policy_timezone: &str,
        base_micropoints: i64,
        carry_micropoints: i64,
    ) -> Result<Self, PolicyError> {
        if base_micropoints < 0 || carry_micropoints < 0 {
            return Err(PolicyError::InvalidBaseUnits);
        }
        let policy_timezone = policy_timezone
            .parse()
            .map_err(|_| PolicyError::InvalidTimezone)?;
        Ok(Self {
            policy_revision_id,
            policy_timezone,
            base_micropoints,
            carry_micropoints,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DailyPolicyStatus {
    Unknown,
    Normal,
    Warning,
    Exceeded,
    Finalized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdTransition {
    Warning,
    Exceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDayProjection {
    pub local_date: NaiveDate,
    pub policy_revision_id: u64,
    pub policy_timezone: Tz,
    pub base_micropoints: i64,
    pub carry_micropoints: i64,
    pub limit_micropoints: i64,
    pub used_micropoints: Option<i64>,
    pub status: DailyPolicyStatus,
    pub threshold_transition: Option<ThresholdTransition>,
    pub finalized: bool,
}

pub(crate) struct PolicyWindowFacts<'a> {
    pub anchor: NaiveDate,
    pub last: NaiveDate,
    pub today: NaiveDate,
    pub policy_revision_id: u64,
    pub usage: &'a BTreeMap<String, Option<i64>>,
    pub snapshots: &'a mut BTreeMap<String, DailyLimitSnapshot>,
    pub previous_statuses: &'a mut BTreeMap<String, DailyPolicyStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("a quota policy must contain exactly seven daily values")]
    InvalidDayCount,
    #[error("daily base quota must be within zero and one full weekly quota")]
    InvalidBaseUnits,
    #[error("seven-day base quota total cannot exceed one full weekly quota")]
    BaseTotalExceeded,
    #[error("policy timezone must be a valid IANA timezone")]
    InvalidTimezone,
    #[error("daily policy date range overflow")]
    DateRangeOverflow,
}

impl QuotaPolicy {
    /// Validates a complete seven-day policy draft atomically.
    ///
    /// # Errors
    ///
    /// Rejects out-of-range daily units, a total over 100%, or an unknown IANA
    /// timezone.
    pub fn new(
        base_micropoints: [i64; 7],
        carry_workdays_enabled: bool,
        policy_timezone: &str,
    ) -> Result<Self, PolicyError> {
        if base_micropoints
            .iter()
            .any(|value| !(0..=100_000_000).contains(value))
        {
            return Err(PolicyError::InvalidBaseUnits);
        }
        let total = base_micropoints
            .iter()
            .try_fold(0_i64, |sum, value| sum.checked_add(*value))
            .ok_or(PolicyError::BaseTotalExceeded)?;
        if total > 100_000_000 {
            return Err(PolicyError::BaseTotalExceeded);
        }
        let policy_timezone = policy_timezone
            .parse()
            .map_err(|_| PolicyError::InvalidTimezone)?;
        Ok(Self {
            base_micropoints,
            carry_workdays_enabled,
            policy_timezone,
        })
    }

    /// Creates the confirmed default Monday-through-Sunday template.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError::InvalidTimezone`] for an unknown IANA name.
    pub fn default_for_timezone(policy_timezone: &str) -> Result<Self, PolicyError> {
        Self::new(DEFAULT_BASE_MICROPOINTS, true, policy_timezone)
    }

    #[must_use]
    pub const fn base_micropoints(&self) -> [i64; 7] {
        self.base_micropoints
    }

    #[must_use]
    pub const fn carry_workdays_enabled(&self) -> bool {
        self.carry_workdays_enabled
    }

    #[must_use]
    pub const fn policy_timezone(&self) -> Tz {
        self.policy_timezone
    }

    /// Returns a notification boundary only for a newly crossed threshold.
    #[must_use]
    pub fn threshold_transition(
        previous: DailyPolicyStatus,
        current: DailyPolicyStatus,
    ) -> Option<ThresholdTransition> {
        match current {
            DailyPolicyStatus::Warning
                if !matches!(
                    previous,
                    DailyPolicyStatus::Warning | DailyPolicyStatus::Exceeded
                ) =>
            {
                Some(ThresholdTransition::Warning)
            }
            DailyPolicyStatus::Exceeded if previous != DailyPolicyStatus::Exceeded => {
                Some(ThresholdTransition::Exceeded)
            }
            _ => None,
        }
    }

    /// Projects explainable base, carry, limit, and status facts in date order.
    ///
    /// Callers include every preceding date needed to establish the natural
    /// week's carry bank, then select the dashboard dates they need.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError::InvalidBaseUnits`] when a usage fact is negative.
    pub fn project_days(
        &self,
        facts: &[PolicyDayFact],
        today: NaiveDate,
        policy_revision_id: u64,
    ) -> Result<Vec<PolicyDayProjection>, PolicyError> {
        let mut projections = Vec::with_capacity(facts.len());
        let mut carry_bank = 0_i64;
        let mut previous_week = None;
        for fact in facts {
            if fact.used_micropoints.is_some_and(|used| used < 0) {
                return Err(PolicyError::InvalidBaseUnits);
            }
            let week = fact.local_date.iso_week();
            if previous_week != Some(week) {
                carry_bank = 0;
                previous_week = Some(week);
            }
            let weekday = fact.local_date.weekday().number_from_monday();
            let active_base = self.base_micropoints[usize::try_from(weekday - 1).unwrap_or(0)];
            let workday = weekday <= 5;
            let active_carry = if self.carry_workdays_enabled && workday {
                carry_bank / i64::from(6 - weekday)
            } else {
                0
            };
            let finalized = fact.finalized || fact.local_date < today;
            let (revision, timezone, base, carry) =
                if let Some(snapshot) = fact.snapshot.as_ref().filter(|_| finalized) {
                    (
                        snapshot.policy_revision_id,
                        snapshot.policy_timezone,
                        snapshot.base_micropoints,
                        snapshot.carry_micropoints,
                    )
                } else {
                    (
                        policy_revision_id,
                        self.policy_timezone,
                        active_base,
                        active_carry,
                    )
                };
            let limit = base.saturating_add(carry);
            let status = daily_status(fact.used_micropoints, limit, finalized);
            let threshold_transition = Self::threshold_transition(
                fact.previous_status.unwrap_or(DailyPolicyStatus::Unknown),
                status,
            );
            projections.push(PolicyDayProjection {
                local_date: fact.local_date,
                policy_revision_id: revision,
                policy_timezone: timezone,
                base_micropoints: base,
                carry_micropoints: carry,
                limit_micropoints: limit,
                used_micropoints: fact.used_micropoints,
                status,
                threshold_transition,
                finalized,
            });
            if self.carry_workdays_enabled && workday {
                carry_bank = carry_bank.saturating_sub(carry).max(0);
                if finalized {
                    let unused = fact
                        .used_micropoints
                        .map_or(0, |used| base.saturating_sub(used).max(0));
                    carry_bank = carry_bank.saturating_add(unused);
                }
            }
        }
        Ok(projections)
    }
}

impl QuotaLedger {
    pub(crate) fn default_policy(policy_timezone: &str) -> Result<QuotaPolicy, PolicyError> {
        QuotaPolicy::default_for_timezone(policy_timezone)
    }

    pub(crate) fn validate_policy(
        base_micropoints: [i64; 7],
        carry_workdays_enabled: bool,
        policy_timezone: &str,
    ) -> Result<QuotaPolicy, PolicyError> {
        QuotaPolicy::new(base_micropoints, carry_workdays_enabled, policy_timezone)
    }

    pub(crate) fn project_policy_days(
        policy: &QuotaPolicy,
        window: &mut PolicyWindowFacts<'_>,
    ) -> Result<Vec<PolicyDayProjection>, PolicyError> {
        let mut facts = Vec::new();
        let mut date = window.anchor;
        loop {
            let key = date.to_string();
            let used = window.usage.get(&key).copied().flatten();
            let fact = if let Some(snapshot) = window.snapshots.remove(&key) {
                PolicyDayFact::with_snapshot(date, used, snapshot)
            } else {
                PolicyDayFact::new(date, used, date < window.today)
            };
            facts.push(match window.previous_statuses.remove(&key) {
                Some(status) => fact.with_previous_status(status),
                None => fact,
            });
            if date == window.last {
                break;
            }
            date = date.succ_opt().ok_or(PolicyError::DateRangeOverflow)?;
        }
        policy.project_days(&facts, window.today, window.policy_revision_id)
    }
}

fn daily_status(
    used_micropoints: Option<i64>,
    limit_micropoints: i64,
    finalized: bool,
) -> DailyPolicyStatus {
    let Some(used) = used_micropoints else {
        return DailyPolicyStatus::Unknown;
    };
    if finalized {
        return DailyPolicyStatus::Finalized;
    }
    if limit_micropoints == 0 {
        return if used > 0 {
            DailyPolicyStatus::Exceeded
        } else {
            DailyPolicyStatus::Normal
        };
    }
    if used.saturating_mul(100) >= limit_micropoints.saturating_mul(100) {
        DailyPolicyStatus::Exceeded
    } else if used.saturating_mul(100) >= limit_micropoints.saturating_mul(80) {
        DailyPolicyStatus::Warning
    } else {
        DailyPolicyStatus::Normal
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Days, NaiveDate};
    use proptest::prelude::*;

    use super::*;

    fn monday() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 7, 27).expect("valid Monday")
    }

    #[test]
    fn default_policy_is_the_confirmed_seven_day_template() {
        let policy = QuotaPolicy::default_for_timezone("Asia/Shanghai").expect("default policy");

        assert_eq!(policy.base_micropoints(), DEFAULT_BASE_MICROPOINTS);
        assert!(policy.carry_workdays_enabled());
        assert_eq!(policy.policy_timezone().name(), "Asia/Shanghai");
    }

    #[test]
    fn threshold_notifications_only_fire_when_a_boundary_is_crossed() {
        assert_eq!(
            QuotaPolicy::threshold_transition(
                DailyPolicyStatus::Normal,
                DailyPolicyStatus::Warning,
            ),
            Some(ThresholdTransition::Warning),
        );
        assert_eq!(
            QuotaPolicy::threshold_transition(
                DailyPolicyStatus::Warning,
                DailyPolicyStatus::Warning,
            ),
            None,
        );
        assert_eq!(
            QuotaPolicy::threshold_transition(
                DailyPolicyStatus::Warning,
                DailyPolicyStatus::Exceeded,
            ),
            Some(ThresholdTransition::Exceeded),
        );
        assert_eq!(
            QuotaPolicy::threshold_transition(
                DailyPolicyStatus::Exceeded,
                DailyPolicyStatus::Exceeded,
            ),
            None,
        );
    }

    proptest! {
        #[test]
        fn validation_accepts_exactly_the_bounded_weekly_domain(
            bases in prop::array::uniform7(0_i64..=100_000_001_i64),
        ) {
            let result = QuotaPolicy::new(bases, true, "Asia/Shanghai");
            let expected_valid = bases.iter().all(|value| *value <= 100_000_000)
                && bases.iter().sum::<i64>() <= 100_000_000;
            prop_assert_eq!(result.is_ok(), expected_valid);
        }

        #[test]
        fn confirmed_monday_unused_quota_is_conserved_across_later_workdays(
            monday_used in 0_i64..=16_000_000_i64,
        ) {
            let policy = QuotaPolicy::default_for_timezone("Asia/Shanghai")
                .expect("valid default policy");
            let monday = monday();
            let facts = (0_u64..5)
                .map(|offset| {
                    PolicyDayFact::new(
                        monday.checked_add_days(Days::new(offset)).expect("date in range"),
                        (offset == 0).then_some(monday_used),
                        offset == 0,
                    )
                })
                .collect::<Vec<_>>();
            let projected = policy
                .project_days(&facts, monday.succ_opt().expect("Tuesday"), 1)
                .expect("valid projection");
            let allocated = projected[1..]
                .iter()
                .map(|day| day.carry_micropoints)
                .sum::<i64>();

            prop_assert_eq!(allocated, 16_000_000 - monday_used);
        }

        #[test]
        fn allocated_carry_is_never_minted_again_by_the_next_finalized_day(
            monday_used in 0_i64..=16_000_000_i64,
            tuesday_used in 0_i64..=16_000_000_i64,
        ) {
            let policy = QuotaPolicy::default_for_timezone("Asia/Shanghai")
                .expect("valid default policy");
            let monday = monday();
            let facts = (0_u64..5)
                .map(|offset| {
                    PolicyDayFact::new(
                        monday.checked_add_days(Days::new(offset)).expect("date in range"),
                        match offset {
                            0 => Some(monday_used),
                            1 => Some(tuesday_used),
                            _ => None,
                        },
                        offset <= 1,
                    )
                })
                .collect::<Vec<_>>();
            let projected = policy
                .project_days(
                    &facts,
                    monday.checked_add_days(Days::new(2)).expect("Wednesday"),
                    1,
                )
                .expect("valid projection");
            let allocated = projected[1..]
                .iter()
                .map(|day| day.carry_micropoints)
                .sum::<i64>();

            prop_assert_eq!(
                allocated,
                (16_000_000 - monday_used) + (16_000_000 - tuesday_used)
            );
        }

        #[test]
        fn completed_snapshot_is_stable_across_policy_and_timezone_edits(
            used in 0_i64..=100_000_000_i64,
            snapshot_base in 0_i64..=100_000_000_i64,
            snapshot_carry in 0_i64..=100_000_000_i64,
        ) {
            let active = QuotaPolicy::new(
                [20_000_000, 20_000_000, 20_000_000, 20_000_000, 0, 10_000_000, 10_000_000],
                true,
                "America/New_York",
            )
            .expect("valid active policy");
            let monday = monday();
            let snapshot = DailyLimitSnapshot::new(
                7,
                "Asia/Shanghai",
                snapshot_base,
                snapshot_carry,
            )
            .expect("valid snapshot");
            let projected = active
                .project_days(
                    &[PolicyDayFact::with_snapshot(monday, Some(used), snapshot)],
                    monday.succ_opt().expect("Tuesday"),
                    8,
                )
                .expect("valid projection");

            prop_assert_eq!(projected[0].policy_revision_id, 7);
            prop_assert_eq!(projected[0].policy_timezone.name(), "Asia/Shanghai");
            prop_assert_eq!(projected[0].base_micropoints, snapshot_base);
            prop_assert_eq!(projected[0].carry_micropoints, snapshot_carry);
        }

        #[test]
        fn unknown_overuse_weekends_and_new_weeks_never_mint_carry(
            overuse in 16_000_000_i64..=100_000_000_i64,
        ) {
            let policy = QuotaPolicy::default_for_timezone("Asia/Shanghai")
                .expect("valid policy");
            let monday = monday();
            let facts = [
                PolicyDayFact::new(monday, Some(overuse), true),
                PolicyDayFact::new(
                    monday.checked_add_days(Days::new(1)).expect("Tuesday"),
                    None,
                    true,
                ),
                PolicyDayFact::new(
                    monday.checked_add_days(Days::new(5)).expect("Saturday"),
                    Some(0),
                    true,
                ),
                PolicyDayFact::new(
                    monday.checked_add_days(Days::new(7)).expect("next Monday"),
                    None,
                    false,
                ),
            ];
            let projected = policy
                .project_days(
                    &facts,
                    monday.checked_add_days(Days::new(7)).expect("next Monday"),
                    1,
                )
                .expect("valid projection");

            prop_assert_eq!(projected[1].carry_micropoints, 0);
            prop_assert_eq!(projected[2].carry_micropoints, 0);
            prop_assert_eq!(projected[3].carry_micropoints, 0);
        }
    }

    #[test]
    fn known_zero_usage_on_a_zero_limit_is_normal() {
        let policy =
            QuotaPolicy::new([0; 7], false, "Asia/Shanghai").expect("zero policy is valid");
        let today = monday();
        let projected = policy
            .project_days(&[PolicyDayFact::new(today, Some(0), false)], today, 1)
            .expect("valid projection");

        assert_eq!(projected[0].status, DailyPolicyStatus::Normal);
    }
}
