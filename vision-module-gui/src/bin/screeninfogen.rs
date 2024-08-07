use app_dirs2::{get_app_root, AppDataType};
use nalgebra::{ComplexField, Point3, RealField, Scalar};
use vision_module_gui::ScreenInfo;
use vision_module_gui::consts::APP_INFO;

pub const MARKER_DEPTH_METERS: f64 = 0.0117;

/// Screen is centered at (0, 0, 0)
/// ```text
/// +--x
/// |
/// y    0    3    4
///
///      1    5    2
/// ```
pub fn marker_pattern<F>(screen_dimensions_meters: [F; 2]) -> [Point3<F>; 6]
where
    F: Scalar + std::ops::SubAssign + ComplexField + RealField + Copy,
{
    let _d = F::from_f64(MARKER_DEPTH_METERS).unwrap();

    let w = screen_dimensions_meters[0];
    let h = screen_dimensions_meters[1];

    // Define the points using ratios
    // [
    //     Point3::from([F::from_f64(0.2  - 0.5).unwrap() * w, F::from_f64(0.15 - 0.5).unwrap() * h, _d]),
    //     Point3::from([F::from_f64(0.25 - 0.5).unwrap() * w, F::from_f64(0.87 - 0.5).unwrap() * h, _d]),
    //     Point3::from([F::from_f64(0.75 - 0.5).unwrap() * w, F::from_f64(0.83 - 0.5).unwrap() * h, _d]),
    //     Point3::from([F::from_f64(0.46 - 0.5).unwrap() * w, F::from_f64(0.2  - 0.5).unwrap() * h, _d]),
    //     Point3::from([F::from_f64(0.7  - 0.5).unwrap() * w, F::from_f64(0.1  - 0.5).unwrap() * h, _d]),
    //     Point3::from([F::from_f64(0.5  - 0.5).unwrap() * w, F::from_f64(0.8  - 0.5).unwrap() * h, _d]),
    // ]

    // for serious wall
    [
        Point3::from([F::from_f64(-0.31944444444).unwrap() * w, F::from_f64(-0.20238095238).unwrap() * h, _d]),
        Point3::from([F::from_f64(-0.34722222222).unwrap() * w, F::from_f64(0.32142857142).unwrap() * h, _d]),
        Point3::from([F::from_f64(0.27083333333).unwrap() * w, F::from_f64(0.28571428571).unwrap() * h, _d]),
        Point3::from([F::from_f64(0.01041666666).unwrap() * w, F::from_f64(-0.15476190476).unwrap() * h, _d]),
        Point3::from([F::from_f64(0.29861111111).unwrap() * w, F::from_f64(0.25  - 0.5).unwrap() * h, _d]),
        Point3::from([F::from_f64(-0.01041666666).unwrap() * w, F::from_f64(0.2619047619).unwrap() * h, _d]),
    ]
}

pub fn main() {
    // 3840x2160 (16:9) SVT
    // let screen_dimensions_meters = [3.64631, 2.05105];

    // 1920x1080 abe's wall
    // let screen_dimensions_meters = [2.28231, 1.2838];

    // serious wall
    let screen_dimensions_meters = [3.6576, 2.1336];

    let pattern = marker_pattern(screen_dimensions_meters);

    let screen_info = ScreenInfo {
        screen_dimensions_meters,
        marker_points: pattern,
    };

    for (i, p) in pattern.iter().enumerate() {
        println!("Marker {} = ({:.4}, {:.4})", i, p.x/0.0254, p.y/0.0254);
    }

    match get_app_root(AppDataType::UserConfig, &APP_INFO)
    .ok()
    .and_then(|config_dir| {
        let screen_info_path = config_dir.join("ats-vision-tool").join("screen-info.json");
        std::fs::create_dir_all(screen_info_path.parent().unwrap()).ok()?;
        serde_json::to_writer_pretty(std::fs::File::create(screen_info_path).ok()?, &screen_info).ok()?;
        Some(screen_info)
    }) {
        Some(screen_info) => {
            println!("Screen info saved to {:?}", screen_info);
        }
        None => {
            println!("Failed to save screen info");
        }
    }
}
