use std::sync::Arc;
use tokio::sync::Mutex;
use iui::controls::{Area, AreaDrawParams, AreaHandler};
use iui::draw::{Brush, FillMode, Path, SolidBrush, StrokeParams};
use iui::{draw, UI};
use crate::custom_shapes::draw_crosshair;
use crate::MotState;

pub struct RunCanvas {
    pub ctx: UI,
    pub state: Arc<Mutex<MotState>>,
}

impl AreaHandler for RunCanvas {
    fn draw(&mut self, _area: &Area, draw_params: &AreaDrawParams) {
        let ctx = &draw_params.context;

        let ch_path = Path::new(ctx, FillMode::Winding);
        let nf_path = Path::new(ctx, FillMode::Winding);
        let wf_path = Path::new(ctx, FillMode::Winding);
        let state = self.state.blocking_lock();
        if state.nf_data.is_some() {
            for mot_data in state.nf_data.as_ref().expect("Nf data is None") {
                if mot_data.area == 0 {
                    break;
                }
                // todo don't use hardcoded 4096x4096 res assumption
                let x = mot_data.cx as f64 / 4096. * draw_params.area_width;
                let y = mot_data.cy as f64 / 4096. * draw_params.area_height;

                let left = mot_data.boundary_left as f64 / 98. * draw_params.area_width;
                let down = mot_data.boundary_down as f64 / 98. * draw_params.area_height;
                let right = mot_data.boundary_right as f64 / 98. * draw_params.area_width;
                let up = mot_data.boundary_up as f64 / 98. * draw_params.area_height;

                draw_crosshair(&ctx, &ch_path, x, y, 50.);

                nf_path.add_rectangle(
                    ctx,
                    left,
                    up,
                    right-left,
                    down-up,
                );
            }
        }
        nf_path.end(ctx);
        if state.wf_data.is_some() {
            for mot_data in state.wf_data.as_ref().expect("Wf data is None") {
                if mot_data.area == 0 {
                    break;
                }
                // todo don't use hardcoded 4096x4096 res assumption
                let x = mot_data.cx as f64 / 4096. * draw_params.area_width;
                let y = mot_data.cy as f64 / 4096. * draw_params.area_height;

                let left = mot_data.boundary_left as f64 / 98. * draw_params.area_width;
                let down = mot_data.boundary_down as f64 / 98. * draw_params.area_height;
                let right = mot_data.boundary_right as f64 / 98. * draw_params.area_width;
                let up = mot_data.boundary_up as f64 / 98. * draw_params.area_height;

                draw_crosshair(&ctx, &ch_path, x, y, 50.);

                wf_path.add_rectangle(
                    ctx,
                    left,
                    up,
                    right-left,
                    down-up,
                );
            }
        }
        wf_path.end(ctx);

        ch_path.end(ctx);

        let brush = Brush::Solid(SolidBrush {
            r: 1.,
            g: 0.,
            b: 0.,
            a: 0.5,
        });

        ctx.fill(&nf_path, &brush);

        let brush = Brush::Solid(SolidBrush {
            r: 0.,
            g: 0.,
            b: 1.,
            a: 0.5,
        });

        ctx.fill(&wf_path, &brush);

        let brush = Brush::Solid(SolidBrush {
            r: 0.,
            g: 0.,
            b: 0.,
            a: 1.,
        });

        let stroke = StrokeParams {
            cap: 0, // Bevel
            join: 0, // Flat
            thickness: 6.,
            miter_limit: 0.,
            dashes: vec![],
            dash_phase: 0.,
        };

        ctx.stroke(&ch_path, &brush, &stroke);
    }
}