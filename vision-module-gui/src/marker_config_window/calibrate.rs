//! Marker offset calibration tab

use std::sync::Arc;

use ats_cv::get_perspective_transform;
use iui::{controls::{VerticalBox, Window}, UI};
use leptos_reactive::{create_effect, RwSignal, SignalGet, SignalGetUntracked, SignalSet, SignalUpdate, SignalWith};
use nalgebra::{Point2, Vector2};
use parking_lot::Mutex;
use tracing::debug;

use crate::{mot_runner::MotRunner, CloneButShorter};

use super::MarkersSettingsForm;

pub fn create(
    ui: &UI,
    calibrating: RwSignal<bool>,
    mot_runner: Arc<Mutex<MotRunner>>,
    marker_settings: MarkersSettingsForm,
    window: Window
) -> VerticalBox {
    let samples = RwSignal::<Vec<Sample>>::new(vec![]);
    crate::layout! { &ui,
        let vbox = VerticalBox(padded: true) {
            Compact : let hbox = HorizontalBox(padded: true) {
                Compact : let start_button = Button(
                    move || if calibrating.get() {
                        "Cancel calibration"
                    } else {
                        "Start calibration"
                    }
                )
            }
            Compact : let hbox = HorizontalBox(padded: true) {
                Compact : let text = Label(move || samples.with(|s| format!("Samples: {}/4", s.len())))
            }
            Compact : let hbox = HorizontalBox(padded: true) {
                Compact : let collect_button = Button("Collect", enabled: calibrating)
            }
        }
    }

    // Enable/disable the text
    create_effect({
        let (ui, text) = (ui.c(), text.c());
        move |_| {
            let mut text = text.c();
            if calibrating.get() {
                text.enable(&ui);
            } else {
                text.disable(&ui);
            }
        }
    });

    start_button.on_clicked(ui, move |_| {
        if calibrating.get_untracked() {
            // Cancelled
            samples.update(Vec::clear);
            calibrating.set(false);
        } else {
            calibrating.set(true);
        }
    });

    collect_button.on_clicked(ui, {
        let ui = ui.c();
        move |_| {
            let runner = mot_runner.lock();
            let reported_aimpoint = runner.state.nf_aimpoint;
            let true_aimpoint = reported_aimpoint + runner.nf_offset;
            drop(runner);
            samples.update(|s| {
                let sample = Sample { reported_aimpoint, true_aimpoint };
                debug!("{sample:?}");
                s.push(sample);

                if let [s1, s2, s3, s4] = s[..] {
                    s.clear();
                    calibrating.set(false);
                    mot_runner.lock().nf_offset = Vector2::new(0.0, 0.0);

                    // do math
                    let transform = match get_perspective_transform(
                        s1.reported_aimpoint,
                        s2.reported_aimpoint,
                        s3.reported_aimpoint,
                        s4.reported_aimpoint,
                        s1.true_aimpoint,
                        s2.true_aimpoint,
                        s3.true_aimpoint,
                        s4.true_aimpoint,
                    ) {
                        Some(t) => t,
                        None => {
                            window.modal_err(&ui, "Calibration failed", "Singular matrix");
                            return;
                        }
                    };
                    debug!("calculated transformation:{transform}");
                    // TODO don't assume view[0]
                    let mut runner = mot_runner.lock();
                    let view = &mut runner.markers_settings.views[0];
                    let top = view.marker_top.position;
                    let bottom = view.marker_bottom.position;
                    let left = view.marker_left.position;
                    let right = view.marker_right.position;

                    let top = Point2::new(top.x, top.y).cast::<f64>();
                    let bottom = Point2::new(bottom.x, bottom.y).cast::<f64>();
                    let left = Point2::new(left.x, left.y).cast::<f64>();
                    let right = Point2::new(right.x, right.y).cast::<f64>();

                    // rescale to 0..1
                    let top = top / 2047.0 / 2.0 + Vector2::new(0.5, 0.5);
                    let bottom = bottom / 2047.0 / 2.0 + Vector2::new(0.5, 0.5);
                    let left = left / 2047.0 / 2.0 + Vector2::new(0.5, 0.5);
                    let right = right / 2047.0 / 2.0 + Vector2::new(0.5, 0.5);

                    let top = transform.transform_point(&top);
                    let bottom = transform.transform_point(&bottom);
                    let left = transform.transform_point(&left);
                    let right = transform.transform_point(&right);

                    // rescale to -2047..2047
                    let top = (top - Vector2::new(0.5, 0.5)) * 2.0 * 2047.0;
                    let bottom = (bottom - Vector2::new(0.5, 0.5)) * 2.0 * 2047.0;
                    let left = (left - Vector2::new(0.5, 0.5)) * 2.0 * 2047.0;
                    let right = (right - Vector2::new(0.5, 0.5)) * 2.0 * 2047.0;

                    view.marker_top.position.x = top.x.round() as i32;
                    view.marker_top.position.y = top.y.round() as i32;
                    view.marker_bottom.position.x = bottom.x.round() as i32;
                    view.marker_bottom.position.y = bottom.y.round() as i32;
                    view.marker_left.position.x = left.x.round() as i32;
                    view.marker_left.position.y = left.y.round() as i32;
                    view.marker_right.position.x = right.x.round() as i32;
                    view.marker_right.position.y = right.y.round() as i32;

                    marker_settings.marker_top.0.set(top.x.round() as i32);
                    marker_settings.marker_top.1.set(top.y.round() as i32);
                    marker_settings.marker_bottom.0.set(bottom.x.round() as i32);
                    marker_settings.marker_bottom.1.set(bottom.y.round() as i32);
                    marker_settings.marker_left.0.set(left.x.round() as i32);
                    marker_settings.marker_left.1.set(left.y.round() as i32);
                    marker_settings.marker_right.0.set(right.x.round() as i32);
                    marker_settings.marker_right.1.set(right.y.round() as i32);
                }
            });
        }
    });
    vbox
}

#[derive(Copy, Clone, Debug)]
struct Sample {
    reported_aimpoint: Point2<f64>,
    true_aimpoint: Point2<f64>,
}
