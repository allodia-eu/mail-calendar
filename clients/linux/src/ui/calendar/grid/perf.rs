//! Release-profile presentation timing for the packaged-runtime calendar qualification.

use std::{cell::Cell, path::PathBuf, rc::Rc, time::Duration};

use adw::prelude::*;

use super::GridScene;

const WARMUP_FRAMES: u32 = 60;
const SAMPLE_FRAMES: u32 = 600;
const SETTLE_FRAMES: u32 = 12;

pub(super) fn start_if_requested(
    drawing: &gtk::DrawingArea,
    adjustment: &gtk::Adjustment,
    scene: &Rc<std::cell::RefCell<GridScene>>,
    started: &Cell<bool>,
    semantic_nodes: bool,
) {
    let Some(path) = std::env::var_os("MAILCAL_CALENDAR_PERF_RESULT").map(PathBuf::from) else {
        return;
    };
    if started.get() || !scene.borrow().is_materialized {
        return;
    }
    started.set(true);
    let drawing = drawing.clone();
    let adjustment = adjustment.clone();
    let events = scene.borrow().events.len();
    gtk::glib::timeout_add_local_once(Duration::from_secs(2), move || {
        install(&drawing, &adjustment, path, events, semantic_nodes);
    });
}

fn install(
    drawing: &gtk::DrawingArea,
    adjustment: &gtk::Adjustment,
    path: PathBuf,
    events: usize,
    semantic_nodes: bool,
) {
    let frame = Rc::new(Cell::new(0_u32));
    let last_counter = Rc::new(Cell::new(-1_i64));
    let presentations = Rc::new(std::cell::RefCell::new(Vec::<i64>::new()));
    let refresh_interval = Rc::new(Cell::new(0_i64));
    let frame_state = Rc::clone(&frame);
    let counter_state = Rc::clone(&last_counter);
    let presentation_state = Rc::clone(&presentations);
    let refresh_state = Rc::clone(&refresh_interval);
    let adjustment = adjustment.clone();
    drawing.add_tick_callback(move |_, clock| {
        let index = frame_state.get();
        let latest = clock.frame_counter().saturating_sub(1);
        let mut counter = counter_state
            .get()
            .saturating_add(1)
            .max(clock.history_start());
        while counter <= latest {
            let Some(timing) = clock.timings(counter) else {
                counter_state.set(counter);
                counter = counter.saturating_add(1);
                continue;
            };
            if !timing.is_complete() {
                break;
            }
            counter_state.set(counter);
            if index > WARMUP_FRAMES && index <= WARMUP_FRAMES + SAMPLE_FRAMES + SETTLE_FRAMES {
                let presented = timing.presentation_time();
                if presented > 0 {
                    presentation_state.borrow_mut().push(presented);
                }
                if timing.refresh_interval() > 0 {
                    refresh_state.set(timing.refresh_interval());
                }
            }
            counter = counter.saturating_add(1);
        }

        if index < WARMUP_FRAMES + SAMPLE_FRAMES {
            move_grid(&adjustment, index);
        }
        let next = index.saturating_add(1);
        frame_state.set(next);
        if next < WARMUP_FRAMES + SAMPLE_FRAMES + SETTLE_FRAMES {
            return gtk::glib::ControlFlow::Continue;
        }

        write_result(
            &path,
            events,
            semantic_nodes,
            refresh_state.get(),
            &presentation_state.borrow(),
        );
        relm4::main_adw_application().quit();
        gtk::glib::ControlFlow::Break
    });
}

#[allow(clippy::cast_precision_loss)]
fn move_grid(adjustment: &gtk::Adjustment, frame: u32) {
    let extent = (adjustment.upper() - adjustment.page_size()).max(0.0);
    let phase = f64::from(frame % 240) / 120.0;
    let position = if phase <= 1.0 { phase } else { 2.0 - phase };
    adjustment.set_value(position * extent);
}

fn write_result(
    path: &PathBuf,
    events: usize,
    semantic_nodes: bool,
    refresh_interval: i64,
    times: &[i64],
) {
    let mut gaps = presentation_gaps(times);
    gaps.sort_unstable();
    let dropped = gaps
        .iter()
        .filter(|gap| refresh_interval > 0 && **gap * 2 > refresh_interval * 3)
        .count();
    let result = serde_json::json!({
        "optimized": !cfg!(debug_assertions),
        "events_in_week": events,
        "semantic_nodes": semantic_nodes,
        "presentation_samples": times.len(),
        "refresh_interval_us": refresh_interval,
        "median_gap_us": percentile(&gaps, 50),
        "p90_gap_us": percentile(&gaps, 90),
        "p99_gap_us": percentile(&gaps, 99),
        "dropped_frames": dropped,
        "measured_gaps": gaps.len(),
        "presentation_times_us": times,
        "gtk": format!("{}.{}.{}", gtk::major_version(), gtk::minor_version(), gtk::micro_version()),
    });
    if let Ok(json) = serde_json::to_vec_pretty(&result) {
        let _ = std::fs::write(path, json);
    }
}

fn presentation_gaps(times: &[i64]) -> Vec<i64> {
    times
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .filter(|gap| *gap > 0)
        .collect()
}

fn percentile(values: &[i64], percentile: usize) -> i64 {
    if values.is_empty() {
        return 0;
    }
    let index = values
        .len()
        .saturating_mul(percentile)
        .div_ceil(100)
        .saturating_sub(1)
        .min(values.len() - 1);
    values[index]
}

#[cfg(test)]
mod tests {
    use super::{percentile, presentation_gaps};

    #[test]
    fn percentiles_select_the_observed_gap_at_the_requested_rank() {
        let values = (1..=100).collect::<Vec<_>>();
        assert_eq!(percentile(&values, 50), 50);
        assert_eq!(percentile(&values, 90), 90);
        assert_eq!(percentile(&values, 99), 99);
        assert_eq!(percentile(&[], 99), 0);
    }

    #[test]
    fn presentation_gaps_keep_long_stalls() {
        assert_eq!(
            presentation_gaps(&[100, 200, 60_200, 60_200, 120_201]),
            vec![100, 60_000, 60_001]
        );
    }
}
