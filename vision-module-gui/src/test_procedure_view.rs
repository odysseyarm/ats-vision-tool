use std::f64::consts::PI;
use iui::controls::{Area, AreaDrawParams, AreaHandler, AreaKeyEvent, Window};
use iui::draw::{Brush, FillMode, Path, SolidBrush};
use iui::UI;

pub struct TestProcedureView {
    pub area: Area,
}

impl TestProcedureView {
    pub fn new(ui: UI, window: Window, on_closing: Box<dyn FnMut(&mut Window)>) -> Self {
        TestProcedureView {
            area: Area::new(&ui, Box::new(TestCanvas { ctx: ui.clone(), window, on_closing })),
        }
    }
}

struct TestCanvas {
    ctx: UI,
    window: Window,
    on_closing: Box<dyn FnMut(&mut Window)>,
}

impl AreaHandler for TestCanvas {
    fn draw(&mut self, _area: &Area, draw_params: &AreaDrawParams) {
        let ctx = &draw_params.context;

        let path = Path::new(ctx, FillMode::Winding);
        path.add_rectangle(ctx, 0., 0., draw_params.area_width, draw_params.area_height);
        path.end(ctx);

        let brush = Brush::Solid(SolidBrush {
            r: 0.2,
            g: 0.6,
            b: 0.8,
            a: 1.,
        });

        draw_params.context.fill(&path, &brush);

        let path = Path::new(ctx, FillMode::Winding);
        for i in 0..100 {
            let x = i as f64 / 100.;
            let y = ((x * PI * 2.).sin() + 1.) / 2.;
            path.add_rectangle(
                ctx,
                x * draw_params.area_width,
                0.,
                draw_params.area_width / 100.,
                y * draw_params.area_height,
            );
        }
        path.end(ctx);

        let brush = Brush::Solid(SolidBrush {
            r: 0.2,
            g: 0.,
            b: 0.3,
            a: 1.,
        });

        draw_params.context.fill(&path, &brush);
    }

    fn key_event(&mut self, _area: &Area, _area_key_event: &AreaKeyEvent) -> bool {
        (self.on_closing)(&mut self.window);
        true
    }
}
