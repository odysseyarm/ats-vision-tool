use std::sync::Arc;
use nalgebra::{Point2, Scale2};
use parking_lot::Mutex;
use iui::controls::{Area, AreaDrawParams, AreaHandler, AreaKeyEvent, Modifiers, Window};
use iui::draw::{Brush, FillMode, Path, SolidBrush, StrokeParams};
use iui::UI;
use tracing::debug;
use crate::custom_shapes::{draw_crosshair, draw_grid, draw_text};
use crate::mot_runner::MotRunner;

pub struct TestCanvas {
    pub ctx: UI,
    pub window: Window,
    pub on_closing: Box<dyn FnMut(&mut Window)>,
    pub runner: Arc<Mutex<MotRunner>>,
    pub last_draw_width: Option<f64>,
    pub last_draw_height: Option<f64>,
}

impl AreaHandler for TestCanvas {
    fn draw(&mut self, _area: &Area, draw_params: &AreaDrawParams) {
        self.last_draw_width = Some(draw_params.area_width);
        self.last_draw_height = Some(draw_params.area_height);
        let ctx = &draw_params.context;

        let background = Path::new(ctx, FillMode::Winding);
        background.add_rectangle(ctx, 0., 0., draw_params.area_width, draw_params.area_height);
        background.end(ctx);

        ctx.fill(&background, &Brush::Solid(SolidBrush {
            r: 0.5,
            g: 0.5,
            b: 0.5,
            a: 1.,
        }));

        let fv_ch_path = Path::new(ctx, FillMode::Winding);
        let runner = self.runner.lock();
        let state = &runner.state;
        {
            let aimpoint = state.fv_aimpoint;
            draw_crosshair(&ctx, &fv_ch_path, aimpoint.x*draw_params.area_width, aimpoint.y*draw_params.area_height, 30.);
        }
        fv_ch_path.end(ctx);
        draw_text(
            &ctx,
            20.0,
            20.0,
            &format!("offset = ({:.4}, {:.4})", runner.fv_offset.x, runner.fv_offset.y),
        );
        draw_text(
            &ctx,
            20.0,
            60.0,
            &format!("screen_id = {}", runner.state.fv_state.screen_id),
        );

        let grid_path = Path::new(ctx, FillMode::Winding);

        // todo lol... i know
        let transform = ats_cv::get_perspective_transform(
            Point2::new(draw_params.area_width/2.0, draw_params.area_height as f64), // bottom
            Point2::new(0.0, draw_params.area_height/2.0), // left
            Point2::new(draw_params.area_width/2.0, 0.0), // top
            Point2::new(draw_params.area_width as f64, draw_params.area_height/2.0), // right
            Point2::new(0.5, 1.), // bottom
            Point2::new(0., 0.5), // left
            Point2::new(0.5, 0.), // top
            Point2::new(1., 0.5), // right
        );
        if let Some(transform) = transform.and_then(|t| t.try_inverse()) {
            draw_grid(ctx, &grid_path, 10, 10, transform);
        }
        grid_path.end(ctx);

        let stroke = StrokeParams {
            cap: 0, // Bevel
            join: 0, // Flat
            thickness: 10.,
            miter_limit: 0.,
            dashes: vec![],
            dash_phase: 0.,
        };

        let brush = Brush::Solid(SolidBrush {
            r: 0.,
            g: 1.,
            b: 0.,
            a: 1.,
        });

        ctx.stroke(&fv_ch_path, &brush, &stroke);

        let stroke = StrokeParams {
            cap: 0, // Bevel
            join: 0, // Flat
            thickness: 5.,
            miter_limit: 0.,
            dashes: vec![],
            dash_phase: 0.,
        };

        // Grid
        let brush = Brush::Solid(SolidBrush {
            r: 0.5,
            g: 0.,
            b: 0.,
            a: 1.,
        });
        let stroke = StrokeParams {
            cap: 0, // Bevel
            join: 0, // Flat
            thickness: 1.,
            miter_limit: 0.,
            dashes: vec![],
            dash_phase: 0.,
        };
        ctx.stroke(&grid_path, &brush, &stroke);
    }

    fn key_event(&mut self, _area: &Area, area_key_event: &AreaKeyEvent) -> bool {
        debug!("{:?}", area_key_event);
        if area_key_event.up {
            return true;
        }
        let mut slow_speed = 0.001;
        if area_key_event.modifiers.contains(Modifiers::MODIFIER_SHIFT) {
            slow_speed = 0.0001;
        }
        match area_key_event.ext_key as _ {
            ui_sys::uiExtKeyUp => self.runner.lock().fv_offset.y -= slow_speed,
            ui_sys::uiExtKeyDown => self.runner.lock().fv_offset.y += slow_speed,
            ui_sys::uiExtKeyLeft => self.runner.lock().fv_offset.x -= slow_speed,
            ui_sys::uiExtKeyRight => self.runner.lock().fv_offset.x += slow_speed,
            ui_sys::uiExtKeyEscape => (self.on_closing)(&mut self.window),
            _ => match area_key_event.key {
                b'w' => self.runner.lock().fv_offset.y -= 0.1,
                b's' => self.runner.lock().fv_offset.y += 0.1,
                b'a' => self.runner.lock().fv_offset.x -= 0.1,
                b'd' => self.runner.lock().fv_offset.x += 0.1,
                b'q' => (self.on_closing)(&mut self.window),
                // Backspace
                8 => self.runner.lock().fv_offset = Default::default(),
                _ => (),
            }
        }
        true
    }

    fn mouse_event(&mut self, _area: &Area, mouse_event: &iui::controls::AreaMouseEvent) {
        if mouse_event.down == 1 && mouse_event.modifiers.contains(Modifiers::MODIFIER_CTRL) {
            let Some(w) = self.last_draw_width else { return };
            let Some(h) = self.last_draw_height else { return };
            let mut state = self.runner.lock();
            let aimpoint = state.state.fv_aimpoint;
            state.fv_offset.x = mouse_event.x / w - aimpoint.x;
            state.fv_offset.y = mouse_event.y / h - aimpoint.y;
        }
    }
}
