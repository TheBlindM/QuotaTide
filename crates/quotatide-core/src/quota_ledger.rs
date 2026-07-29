use std::collections::BTreeMap;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use chrono_tz::Tz;

use crate::{QuotaUnits, WeeklyUsageObservation};

const WEEK_SECONDS: u32 = 604_800;
const RESET_DROP_MICROPOINTS: i64 = 10_000;
const RESET_ADVANCE_TOLERANCE_SECONDS: i64 = 60;
const MAX_SCHEDULE_CORRECTION_SECONDS: u64 = 60 * 60;

/// Pure current-account quota ledger.
pub struct QuotaLedger;

/// Persistable state for one isolated account stream.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LedgerState {
    active_epoch: Option<QuotaEpoch>,
    reset_candidate: Option<ResetCandidate>,
    schedule_candidate: Option<ScheduleCandidate>,
    daily_used_micropoints: BTreeMap<NaiveDate, i64>,
    next_epoch_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PersistedLedgerEpoch {
    pub sequence: u64,
    pub baseline_micropoints: i64,
    pub high_water_micropoints: i64,
    pub first_observed_at_unix_ms: i64,
    pub latest_observed_at_unix_ms: i64,
    pub scheduled_reset_at_unix_s: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotaEpoch {
    sequence: u64,
    baseline: QuotaUnits,
    high_water: QuotaUnits,
    first_observed_at_unix_ms: i64,
    latest_observed_at_unix_ms: i64,
    scheduled_reset_at_unix_s: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResetCandidate {
    resets_at_unix_s: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScheduleCandidate {
    resets_at_unix_s: i64,
}

/// Meaning of an accepted ledger transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerApplyKind {
    Baseline,
    SameEpoch,
    ConfirmedReset,
    DroppedOutOfOrder,
}

/// Pure result to be persisted atomically with its observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerTransition {
    pub state: LedgerState,
    pub kind: LedgerApplyKind,
    pub added_micropoints: i64,
    pub assigned_local_date: Option<String>,
}

/// Seven-day semantic projection for the current epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerProjection {
    pub epoch_sequence: u64,
    pub window_starts_on: String,
    pub window_ends_on: String,
    pub days: Vec<DailyUsageFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyUsageFact {
    pub local_date: String,
    pub used_micropoints: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LedgerError {
    #[error("quota ledger requires the exact current weekly window")]
    InvalidWindow,
    #[error("quota ledger observation time is invalid")]
    InvalidObservationTime,
    #[error("quota ledger reset time is invalid")]
    InvalidResetTime,
    #[error("quota ledger date range overflow")]
    DateRangeOverflow,
}

impl QuotaLedger {
    pub(crate) fn persisted_epoch(state: &LedgerState) -> Option<PersistedLedgerEpoch> {
        state
            .active_epoch
            .as_ref()
            .map(|epoch| PersistedLedgerEpoch {
                sequence: epoch.sequence,
                baseline_micropoints: epoch.baseline.micropoints(),
                high_water_micropoints: epoch.high_water.micropoints(),
                first_observed_at_unix_ms: epoch.first_observed_at_unix_ms,
                latest_observed_at_unix_ms: epoch.latest_observed_at_unix_ms,
                scheduled_reset_at_unix_s: epoch.scheduled_reset_at_unix_s,
            })
    }

    pub(crate) fn daily_used(state: &LedgerState, local_date: &str) -> Option<i64> {
        local_date
            .parse::<NaiveDate>()
            .ok()
            .and_then(|date| state.daily_used_micropoints.get(&date).copied())
    }

    /// Applies one normalized observation without performing I/O.
    ///
    /// The first sample establishes a baseline and contributes no inferred
    /// daily use. Only a confirmed reset makes the new epoch's first sample a
    /// daily increment.
    ///
    /// # Errors
    ///
    /// Rejects non-weekly windows or invalid timestamps.
    pub fn apply(
        mut state: LedgerState,
        observation: &WeeklyUsageObservation,
        policy_timezone: Tz,
    ) -> Result<LedgerTransition, LedgerError> {
        let captured = validate_observation(observation)?;

        let Some(epoch) = state.active_epoch.as_mut() else {
            state.next_epoch_sequence = state.next_epoch_sequence.saturating_add(1);
            state.active_epoch = Some(QuotaEpoch {
                sequence: state.next_epoch_sequence,
                baseline: observation.used,
                high_water: observation.used,
                first_observed_at_unix_ms: observation.captured_at_unix_ms,
                latest_observed_at_unix_ms: observation.captured_at_unix_ms,
                scheduled_reset_at_unix_s: observation.resets_at_unix_s,
            });
            return Ok(LedgerTransition {
                state,
                kind: LedgerApplyKind::Baseline,
                added_micropoints: 0,
                assigned_local_date: None,
            });
        };

        if observation.captured_at_unix_ms <= epoch.latest_observed_at_unix_ms {
            return Ok(LedgerTransition {
                state,
                kind: LedgerApplyKind::DroppedOutOfOrder,
                added_micropoints: 0,
                assigned_local_date: None,
            });
        }

        let used = observation.used.micropoints();
        let high_water = epoch.high_water.micropoints();
        let materially_below_high_water = high_water.saturating_sub(used) > RESET_DROP_MICROPOINTS;
        let crossed_old_boundary =
            observation.captured_at_unix_ms >= epoch.scheduled_reset_at_unix_s.saturating_mul(1000);
        let reset_advanced_one_window = observation.resets_at_unix_s
            >= epoch
                .scheduled_reset_at_unix_s
                .saturating_add(i64::from(WEEK_SECONDS))
                .saturating_sub(RESET_ADVANCE_TOLERANCE_SECONDS);
        // A used-value drop is only one anomalous fact. It cannot establish a
        // new epoch by itself: confirmation comes from either the scheduled
        // boundary advancing or a second coherent low observation.
        let candidate_confirmed = state.reset_candidate.as_ref().is_some_and(|candidate| {
            materially_below_high_water
                && observation
                    .resets_at_unix_s
                    .saturating_sub(candidate.resets_at_unix_s)
                    .unsigned_abs()
                    <= MAX_SCHEDULE_CORRECTION_SECONDS
        });
        let schedule_candidate_confirmed =
            state.schedule_candidate.as_ref().is_some_and(|candidate| {
                !materially_below_high_water
                    && observation
                        .resets_at_unix_s
                        .saturating_sub(candidate.resets_at_unix_s)
                        .unsigned_abs()
                        <= MAX_SCHEDULE_CORRECTION_SECONDS
            });
        let confirmed_reset =
            (crossed_old_boundary && reset_advanced_one_window) || candidate_confirmed;

        let (kind, added) = if confirmed_reset {
            let reset_local_date = captured.with_timezone(&policy_timezone).date_naive();
            state
                .daily_used_micropoints
                .retain(|date, _| *date == reset_local_date);
            state.next_epoch_sequence = state.next_epoch_sequence.saturating_add(1);
            state.active_epoch = Some(QuotaEpoch {
                sequence: state.next_epoch_sequence,
                baseline: observation.used,
                high_water: observation.used,
                first_observed_at_unix_ms: observation.captured_at_unix_ms,
                latest_observed_at_unix_ms: observation.captured_at_unix_ms,
                scheduled_reset_at_unix_s: observation.resets_at_unix_s,
            });
            state.reset_candidate = None;
            state.schedule_candidate = None;
            (LedgerApplyKind::ConfirmedReset, used)
        } else {
            let added = apply_same_epoch_observation(
                epoch,
                &mut state.reset_candidate,
                &mut state.schedule_candidate,
                observation,
                high_water,
                materially_below_high_water,
                schedule_candidate_confirmed,
            )?;
            (LedgerApplyKind::SameEpoch, added)
        };

        let assigned_local_date = if added > 0 {
            let date = captured.with_timezone(&policy_timezone).date_naive();
            let entry = state.daily_used_micropoints.entry(date).or_default();
            *entry = entry.saturating_add(added);
            Some(date.to_string())
        } else {
            None
        };
        Ok(LedgerTransition {
            state,
            kind,
            added_micropoints: added,
            assigned_local_date,
        })
    }

    /// Projects exactly seven natural dates for the active current epoch.
    ///
    /// # Errors
    ///
    /// Returns an error if the persisted reset boundary cannot form a date
    /// range.
    pub fn project(
        state: &LedgerState,
        policy_timezone: Tz,
    ) -> Result<Option<LedgerProjection>, LedgerError> {
        let Some(epoch) = state.active_epoch.as_ref() else {
            return Ok(None);
        };
        let reset = DateTime::<Utc>::from_timestamp(epoch.scheduled_reset_at_unix_s, 0)
            .ok_or(LedgerError::InvalidResetTime)?;
        let reset_date = reset.with_timezone(&policy_timezone).date_naive();
        let window_start = reset_date
            .checked_sub_signed(Duration::days(7))
            .ok_or(LedgerError::InvalidResetTime)?;
        let mut days = Vec::with_capacity(7);
        let mut date = window_start;
        for index in 0..7 {
            days.push(DailyUsageFact {
                local_date: date.to_string(),
                used_micropoints: state.daily_used_micropoints.get(&date).copied(),
            });
            if index < 6 {
                date = date.succ_opt().ok_or(LedgerError::DateRangeOverflow)?;
            }
        }
        Ok(Some(LedgerProjection {
            epoch_sequence: epoch.sequence,
            window_starts_on: window_start.to_string(),
            window_ends_on: date.to_string(),
            days,
        }))
    }
}

fn apply_same_epoch_observation(
    epoch: &mut QuotaEpoch,
    reset_candidate: &mut Option<ResetCandidate>,
    schedule_candidate: &mut Option<ScheduleCandidate>,
    observation: &WeeklyUsageObservation,
    high_water: i64,
    materially_below_high_water: bool,
    schedule_candidate_confirmed: bool,
) -> Result<i64, LedgerError> {
    let used = observation.used.micropoints();
    let added = used.saturating_sub(high_water).max(0);
    epoch.high_water =
        QuotaUnits::from_micropoints(high_water.max(used)).ok_or(LedgerError::InvalidWindow)?;
    epoch.latest_observed_at_unix_ms = observation.captured_at_unix_ms;
    let schedule_correction = observation
        .resets_at_unix_s
        .saturating_sub(epoch.scheduled_reset_at_unix_s)
        .unsigned_abs();
    if !materially_below_high_water
        && (schedule_correction <= MAX_SCHEDULE_CORRECTION_SECONDS || schedule_candidate_confirmed)
    {
        epoch.scheduled_reset_at_unix_s = observation.resets_at_unix_s;
        *schedule_candidate = None;
    } else if materially_below_high_water {
        *schedule_candidate = None;
    } else {
        *schedule_candidate = Some(ScheduleCandidate {
            resets_at_unix_s: observation.resets_at_unix_s,
        });
    }
    *reset_candidate = materially_below_high_water.then_some(ResetCandidate {
        resets_at_unix_s: observation.resets_at_unix_s,
    });
    Ok(added)
}

fn validate_observation(
    observation: &WeeklyUsageObservation,
) -> Result<DateTime<Utc>, LedgerError> {
    if observation.window_seconds != WEEK_SECONDS {
        return Err(LedgerError::InvalidWindow);
    }
    let captured = DateTime::<Utc>::from_timestamp_millis(observation.captured_at_unix_ms)
        .ok_or(LedgerError::InvalidObservationTime)?;
    DateTime::<Utc>::from_timestamp(observation.resets_at_unix_s, 0)
        .ok_or(LedgerError::InvalidResetTime)?;
    let window_starts_at_unix_ms = observation
        .resets_at_unix_s
        .saturating_sub(i64::from(WEEK_SECONDS))
        .saturating_mul(1000);
    let window_ends_at_unix_ms = observation.resets_at_unix_s.saturating_mul(1000);
    if observation.captured_at_unix_ms < window_starts_at_unix_ms
        || observation.captured_at_unix_ms >= window_ends_at_unix_ms
    {
        return Err(LedgerError::InvalidWindow);
    }
    Ok(captured)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use chrono_tz::America::New_York;
    use chrono_tz::Asia::Shanghai;

    use super::{LedgerApplyKind, LedgerState, QuotaLedger};
    use crate::{QuotaUnits, WeeklyUsageObservation};

    #[test]
    fn first_sample_is_a_baseline_not_inferred_daily_usage() {
        let transition = QuotaLedger::apply(
            LedgerState::default(),
            &observation(1_700_000_000_000, 42_000_000, 1_700_604_800),
            Shanghai,
        )
        .expect("baseline");

        assert_eq!(transition.kind, LedgerApplyKind::Baseline);
        assert_eq!(transition.added_micropoints, 0);
        assert!(transition.assigned_local_date.is_none());
        assert!(
            QuotaLedger::project(&transition.state, Shanghai)
                .expect("projection")
                .expect("active epoch")
                .days
                .iter()
                .all(|day| day.used_micropoints.is_none())
        );
    }

    #[test]
    fn observation_must_belong_to_its_strict_weekly_window() {
        let result = QuotaLedger::apply(
            LedgerState::default(),
            &observation(1_700_000_000_000, 42_000_000, 1_701_209_600),
            Shanghai,
        );

        assert_eq!(result, Err(super::LedgerError::InvalidWindow));
    }

    #[test]
    fn same_epoch_uses_a_high_water_and_ignores_rounding_regressions() {
        let baseline = apply_default(42_000_000);
        let increase = QuotaLedger::apply(
            baseline.state,
            &observation(1_700_003_600_000, 43_000_000, 1_700_604_800),
            Shanghai,
        )
        .expect("increase");
        let noise = QuotaLedger::apply(
            increase.state,
            &observation(1_700_007_200_000, 42_995_000, 1_700_604_840),
            Shanghai,
        )
        .expect("noise");
        let recovery = QuotaLedger::apply(
            noise.state,
            &observation(1_700_010_800_000, 43_100_000, 1_700_604_860),
            Shanghai,
        )
        .expect("recovery");

        assert_eq!(increase.added_micropoints, 1_000_000);
        assert_eq!(noise.kind, LedgerApplyKind::SameEpoch);
        assert_eq!(noise.added_micropoints, 0);
        assert_eq!(recovery.added_micropoints, 100_000);
    }

    #[test]
    fn a_confirmed_drop_starts_a_new_epoch_and_keeps_same_day_usage() {
        let baseline = QuotaLedger::apply(
            LedgerState::default(),
            &observation(1_700_600_000_000, 50_000_000, 1_700_604_800),
            Shanghai,
        )
        .expect("baseline");
        let before_reset = QuotaLedger::apply(
            baseline.state,
            &observation(1_700_602_000_000, 55_000_000, 1_700_604_800),
            Shanghai,
        )
        .expect("before reset");
        let reset = QuotaLedger::apply(
            before_reset.state,
            &observation(1_700_605_000_000, 2_000_000, 1_701_209_600),
            Shanghai,
        )
        .expect("confirmed reset");

        assert_eq!(reset.kind, LedgerApplyKind::ConfirmedReset);
        assert_eq!(reset.added_micropoints, 2_000_000);
        let total: i64 = QuotaLedger::project(&reset.state, Shanghai)
            .expect("projection")
            .expect("epoch")
            .days
            .iter()
            .filter_map(|day| day.used_micropoints)
            .sum();
        assert_eq!(total, 7_000_000);
    }

    #[test]
    fn reset_text_drift_before_the_boundary_is_only_a_schedule_correction() {
        let baseline = apply_default(42_000_000);
        let corrected = QuotaLedger::apply(
            baseline.state,
            &observation(1_700_003_600_000, 42_000_000, 1_700_604_860),
            Shanghai,
        )
        .expect("schedule correction");

        assert_eq!(corrected.kind, LedgerApplyKind::SameEpoch);
        assert_eq!(corrected.added_micropoints, 0);
    }

    #[test]
    fn a_large_schedule_correction_requires_two_coherent_samples() {
        let baseline = apply_default(42_000_000);
        let candidate = QuotaLedger::apply(
            baseline.state,
            &observation(1_700_007_200_000, 42_500_000, 1_700_612_000),
            Shanghai,
        )
        .expect("schedule candidate");
        assert_eq!(
            QuotaLedger::persisted_epoch(&candidate.state)
                .expect("candidate epoch")
                .scheduled_reset_at_unix_s,
            1_700_604_800
        );

        let confirmed = QuotaLedger::apply(
            candidate.state,
            &observation(1_700_010_800_000, 43_000_000, 1_700_612_020),
            Shanghai,
        )
        .expect("confirmed schedule correction");
        let epoch = QuotaLedger::persisted_epoch(&confirmed.state).expect("confirmed epoch");

        assert_eq!(confirmed.kind, LedgerApplyKind::SameEpoch);
        assert_eq!(epoch.sequence, 1);
        assert_eq!(epoch.scheduled_reset_at_unix_s, 1_700_612_020);
    }

    #[test]
    fn one_large_used_regression_does_not_establish_a_new_epoch() {
        let baseline = apply_default(42_000_000);
        let anomaly = QuotaLedger::apply(
            baseline.state,
            &observation(1_700_003_600_000, 2_000_000, 1_700_604_800),
            Shanghai,
        )
        .expect("single anomalous sample");

        assert_eq!(anomaly.kind, LedgerApplyKind::SameEpoch);
        assert_eq!(anomaly.added_micropoints, 0);
    }

    #[test]
    fn a_second_coherent_low_sample_confirms_a_reset_before_the_old_boundary() {
        let baseline = apply_default(42_000_000);
        let candidate = QuotaLedger::apply(
            baseline.state,
            &observation(1_700_003_600_000, 2_000_000, 1_700_608_400),
            Shanghai,
        )
        .expect("candidate");
        let confirmed = QuotaLedger::apply(
            candidate.state,
            &observation(1_700_007_200_000, 2_500_000, 1_700_608_420),
            Shanghai,
        )
        .expect("second coherent sample");

        assert_eq!(confirmed.kind, LedgerApplyKind::ConfirmedReset);
        assert_eq!(confirmed.added_micropoints, 2_500_000);
    }

    #[test]
    fn an_early_reset_drops_prior_epoch_days_but_keeps_the_reset_day() {
        let baseline = apply_default(42_000_000);
        let prior_day = QuotaLedger::apply(
            baseline.state,
            &observation(1_700_003_600_000, 43_000_000, 1_700_604_800),
            Shanghai,
        )
        .expect("prior-day increase");
        let candidate = QuotaLedger::apply(
            prior_day.state,
            &observation(1_700_090_000_000, 2_000_000, 1_700_604_900),
            Shanghai,
        )
        .expect("early reset candidate");
        let confirmed = QuotaLedger::apply(
            candidate.state,
            &observation(1_700_093_600_000, 2_500_000, 1_700_604_920),
            Shanghai,
        )
        .expect("confirmed early reset");
        let total: i64 = QuotaLedger::project(&confirmed.state, Shanghai)
            .expect("projection")
            .expect("epoch")
            .days
            .iter()
            .filter_map(|day| day.used_micropoints)
            .sum();

        assert_eq!(confirmed.kind, LedgerApplyKind::ConfirmedReset);
        assert_eq!(total, 2_500_000);
    }

    #[test]
    fn a_future_reset_anomaly_cannot_move_the_confirming_boundary() {
        let baseline = apply_default(42_000_000);
        let anomaly = QuotaLedger::apply(
            baseline.state,
            &observation(1_700_003_600_000, 2_000_000, 1_700_608_400),
            Shanghai,
        )
        .expect("future reset anomaly");
        assert_eq!(anomaly.kind, LedgerApplyKind::SameEpoch);
        let confirmed = QuotaLedger::apply(
            anomaly.state,
            &observation(1_700_605_000_000, 3_000_000, 1_701_209_600),
            Shanghai,
        )
        .expect("confirmed at original boundary");

        assert_eq!(confirmed.kind, LedgerApplyKind::ConfirmedReset);
    }

    #[test]
    fn increments_are_assigned_to_the_policy_timezones_natural_date() {
        let before_midnight = DateTime::parse_from_rfc3339("2026-07-28T15:59:00Z")
            .expect("timestamp")
            .timestamp_millis();
        let after_midnight = DateTime::parse_from_rfc3339("2026-07-28T16:01:00Z")
            .expect("timestamp")
            .timestamp_millis();
        let reset = DateTime::parse_from_rfc3339("2026-08-03T16:00:00Z")
            .expect("reset")
            .timestamp();
        let baseline = QuotaLedger::apply(
            LedgerState::default(),
            &observation(before_midnight, 42_000_000, reset),
            Shanghai,
        )
        .expect("baseline");
        let increase = QuotaLedger::apply(
            baseline.state,
            &observation(after_midnight, 43_000_000, reset),
            Shanghai,
        )
        .expect("increase");

        assert_eq!(increase.assigned_local_date.as_deref(), Some("2026-07-29"));
        assert_eq!(increase.added_micropoints, 1_000_000);
    }

    #[test]
    fn property_high_water_deltas_are_never_negative_or_double_counted() {
        for first in (0..=100_000_000).step_by(5_000_000) {
            for second in (0..=100_000_000).step_by(5_000_000) {
                let baseline = apply_default(first);
                let transition = QuotaLedger::apply(
                    baseline.state,
                    &observation(1_700_003_600_000, second, 1_700_604_800),
                    Shanghai,
                )
                .expect("bounded property sample");

                assert_eq!(
                    transition.added_micropoints,
                    second.saturating_sub(first).max(0)
                );
                assert!(transition.added_micropoints >= 0);
            }
        }
    }

    #[test]
    fn projection_is_seven_natural_dates_across_dst_with_unknown_slots() {
        let transition = QuotaLedger::apply(
            LedgerState::default(),
            &observation(1_730_700_000_000, 10_000_000, 1_731_283_200),
            New_York,
        )
        .expect("baseline");
        let projected = QuotaLedger::project(&transition.state, New_York)
            .expect("projection")
            .expect("active epoch");

        assert_eq!(projected.days.len(), 7);
        assert_eq!(
            projected.window_ends_on,
            DateTime::<Utc>::from_timestamp(1_731_283_200, 0)
                .expect("reset")
                .with_timezone(&New_York)
                .date_naive()
                .pred_opt()
                .expect("day before reset")
                .to_string()
        );
        for dates in projected.days.windows(2) {
            let earlier = dates[0]
                .local_date
                .parse::<chrono::NaiveDate>()
                .expect("earlier date");
            let later = dates[1]
                .local_date
                .parse::<chrono::NaiveDate>()
                .expect("later date");
            assert_eq!(later, earlier.succ_opt().expect("next natural date"));
        }
        assert!(
            projected
                .days
                .iter()
                .all(|day| day.used_micropoints.is_none())
        );
    }

    fn apply_default(used: i64) -> super::LedgerTransition {
        QuotaLedger::apply(
            LedgerState::default(),
            &observation(1_700_000_000_000, used, 1_700_604_800),
            Shanghai,
        )
        .expect("baseline")
    }

    fn observation(
        captured_at_unix_ms: i64,
        used_micropoints: i64,
        resets_at_unix_s: i64,
    ) -> WeeklyUsageObservation {
        WeeklyUsageObservation {
            captured_at_unix_ms,
            used: QuotaUnits::from_micropoints(used_micropoints).expect("quota"),
            window_seconds: 604_800,
            resets_at_unix_s,
            plan_type: Some("plus".to_owned()),
            allowed: Some(true),
        }
    }
}
