const ICON_SIZE: u32 = 44;
const SAMPLES_PER_AXIS: u32 = 4;
const FULL_WEEK_MICROPOINTS: u32 = 100_000_000;

pub(crate) const TRAY_METER_ICON_SIZE: u32 = ICON_SIZE;
pub(crate) const TRAY_METER_FRAME_COUNT: u8 = 12;

/// Renders a compact liquid-style weekly usage meter for the native tray.
///
/// The alpha channel carries the shape so macOS can recolor it as a template
/// image. RGB colors remain useful on Windows, where tray template images are
/// not supported.
pub(crate) fn render_weekly_meter_rgba(used_micropoints: u32) -> Vec<u8> {
    render_weekly_meter_frame_rgba(used_micropoints, 0)
}

pub(crate) fn render_weekly_meter_frame_rgba(used_micropoints: u32, frame: u8) -> Vec<u8> {
    let pixel_count = usize::try_from(ICON_SIZE * ICON_SIZE).expect("tray icon dimensions fit");
    let mut rgba = vec![0_u8; pixel_count * 4];
    let sample_count = SAMPLES_PER_AXIS * SAMPLES_PER_AXIS;
    let coordinate_scale = SAMPLES_PER_AXIS * 2;
    let center = ICON_SIZE * SAMPLES_PER_AXIS;
    let outer_radius = 19 * coordinate_scale;
    let ring_inner_radius = 16 * coordinate_scale;
    let fill_radius = 15 * coordinate_scale;
    let used = used_micropoints.min(FULL_WEEK_MICROPOINTS);
    let used_fraction = f64::from(used) / f64::from(FULL_WEEK_MICROPOINTS);
    let fill_height = f64::from(2 * fill_radius) * used_fraction;
    let fill_top = f64::from(center + fill_radius) - fill_height;
    let edge_factor = (used_fraction.min(1.0 - used_fraction) * 4.0).min(1.0);
    let wave_amplitude = f64::from(coordinate_scale) * 1.7 * edge_factor;
    let phase = std::f64::consts::TAU * f64::from(frame % TRAY_METER_FRAME_COUNT)
        / f64::from(TRAY_METER_FRAME_COUNT);

    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let mut ring_coverage = 0_u32;
            let mut fill_coverage = 0_u32;
            for sample_y in 0..SAMPLES_PER_AXIS {
                for sample_x in 0..SAMPLES_PER_AXIS {
                    let sample_x = (x * SAMPLES_PER_AXIS + sample_x) * 2 + 1;
                    let sample_y = (y * SAMPLES_PER_AXIS + sample_y) * 2 + 1;
                    let dx = i64::from(sample_x) - i64::from(center);
                    let dy = i64::from(sample_y) - i64::from(center);
                    let distance_squared = dx * dx + dy * dy;
                    if distance_squared <= i64::from(outer_radius).pow(2)
                        && distance_squared >= i64::from(ring_inner_radius).pow(2)
                    {
                        ring_coverage += 1;
                    }
                    let horizontal_position =
                        f64::from(i32::try_from(dx).expect("tray sample coordinate fits"))
                            / f64::from(2 * fill_radius);
                    let primary_wave =
                        (horizontal_position * std::f64::consts::TAU * 1.35 + phase).sin();
                    let secondary_wave =
                        (horizontal_position * std::f64::consts::TAU * 2.4 - phase * 1.4).sin()
                            * 0.28;
                    let surface = fill_top + wave_amplitude * (primary_wave + secondary_wave);
                    if distance_squared <= i64::from(fill_radius).pow(2)
                        && f64::from(sample_y) >= surface
                    {
                        fill_coverage += 1;
                    }
                }
            }

            let offset =
                usize::try_from((y * ICON_SIZE + x) * 4).expect("tray icon pixel offset fits");
            if fill_coverage > 0 {
                rgba[offset..offset + 4].copy_from_slice(&[
                    70,
                    190,
                    139,
                    coverage_alpha(fill_coverage, sample_count, 255),
                ]);
            } else if ring_coverage > 0 {
                rgba[offset..offset + 4].copy_from_slice(&[
                    91,
                    111,
                    102,
                    coverage_alpha(ring_coverage, sample_count, 210),
                ]);
            }
        }
    }

    rgba
}

fn coverage_alpha(coverage: u32, sample_count: u32, maximum: u32) -> u8 {
    let alpha = coverage * maximum / sample_count;
    u8::try_from(alpha).expect("coverage alpha is bounded to one byte")
}

#[cfg(test)]
mod tests {
    use super::{ICON_SIZE, render_weekly_meter_frame_rgba, render_weekly_meter_rgba};

    fn alpha_at(image: &[u8], x: u32, y: u32) -> u8 {
        let offset = usize::try_from((y * ICON_SIZE + x) * 4).expect("test pixel offset fits");
        image[offset + 3]
    }

    fn opaque_pixel_count(image: &[u8]) -> usize {
        image.chunks_exact(4).filter(|pixel| pixel[3] > 0).count()
    }

    #[test]
    fn weekly_usage_fills_the_circle_from_bottom_to_top() {
        let empty = render_weekly_meter_rgba(0);
        let partial = render_weekly_meter_rgba(29_000_000);
        let full = render_weekly_meter_rgba(100_000_000);

        assert_eq!(
            empty.len(),
            usize::try_from(ICON_SIZE * ICON_SIZE * 4).unwrap()
        );
        assert_eq!(alpha_at(&empty, 22, 35), 0);
        assert!(alpha_at(&partial, 22, 35) > 0);
        assert_eq!(alpha_at(&partial, 22, 12), 0);
        assert!(alpha_at(&full, 22, 12) > 0);
        assert!(opaque_pixel_count(&empty) < opaque_pixel_count(&partial));
        assert!(opaque_pixel_count(&partial) < opaque_pixel_count(&full));
        assert_eq!(alpha_at(&full, 0, 0), 0);
    }

    #[test]
    fn weekly_usage_is_clamped_at_a_full_circle() {
        assert_eq!(
            render_weekly_meter_rgba(100_000_000),
            render_weekly_meter_rgba(u32::MAX),
        );
    }

    #[test]
    fn partial_usage_animates_the_wave_without_moving_empty_or_full_states() {
        let partial_a = render_weekly_meter_frame_rgba(29_000_000, 0);
        let partial_b = render_weekly_meter_frame_rgba(29_000_000, 4);

        assert_ne!(partial_a, partial_b);
        assert_eq!(
            render_weekly_meter_frame_rgba(0, 0),
            render_weekly_meter_frame_rgba(0, 4),
        );
        assert_eq!(
            render_weekly_meter_frame_rgba(100_000_000, 0),
            render_weekly_meter_frame_rgba(100_000_000, 4),
        );
    }
}
