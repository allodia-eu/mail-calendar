//! Initial vertical framing for the time grid.

pub(super) fn centred_scroll_value(
    content_top: f64,
    hour_height: f64,
    now_minutes: u32,
    viewport_height: f64,
    content_height: f64,
) -> Option<f64> {
    if !content_top.is_finite()
        || !hour_height.is_finite()
        || !viewport_height.is_finite()
        || !content_height.is_finite()
        || hour_height <= 0.0
        || viewport_height <= 0.0
        || content_height <= viewport_height
    {
        return None;
    }
    let line = content_top + f64::from(now_minutes.min(24 * 60)) * hour_height / 60.0;
    Some((line - viewport_height / 2.0).clamp(0.0, content_height - viewport_height))
}

#[cfg(test)]
mod tests {
    use super::centred_scroll_value;

    #[test]
    fn current_time_is_placed_at_the_viewport_midpoint() {
        let viewport = 480.0;
        let line = 52.0 + 12.0 * 60.0;
        let value = centred_scroll_value(52.0, 60.0, 12 * 60, viewport, 52.0 + 24.0 * 60.0)
            .expect("a laid-out day can be centred");
        assert!((line - value - viewport / 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_day_edges_clamp_without_exposing_empty_space() {
        assert_eq!(
            centred_scroll_value(52.0, 60.0, 30, 480.0, 52.0 + 24.0 * 60.0),
            Some(0.0)
        );
        assert_eq!(
            centred_scroll_value(52.0, 60.0, 23 * 60 + 30, 480.0, 52.0 + 24.0 * 60.0),
            Some(52.0 + 24.0 * 60.0 - 480.0)
        );
    }

    #[test]
    fn centring_waits_for_real_layout_metrics() {
        assert_eq!(centred_scroll_value(52.0, 60.0, 720, 0.0, 1492.0), None);
        assert_eq!(centred_scroll_value(52.0, 0.0, 720, 480.0, 1492.0), None);
        assert_eq!(centred_scroll_value(52.0, 60.0, 720, 480.0, 480.0), None);
    }
}
