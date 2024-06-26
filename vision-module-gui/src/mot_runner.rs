use std::sync::Arc;
use ahrs::Ahrs;
use ats_cv::calculate_rotational_offset;
use ats_cv::foveated::{identify_markers2, match3};
use ats_cv::kalman::Pva2d;
use opencv_ros_camera::RosOpenCvIntrinsics;
use parking_lot::Mutex;
use std::time::{Duration, UNIX_EPOCH};
use arrayvec::ArrayVec;
use iui::concurrent::Context;
use leptos_reactive::{RwSignal, SignalGetUntracked};
use nalgebra::{Const, Isometry3, Matrix3, Point2, Rotation3, Scalar, Translation3, UnitVector3, Vector2, Vector3};
use sqpnp::types::{SQPSolution, SolverParameters};
use tokio::time::{sleep, Instant};
use tokio_stream::StreamExt;
use tracing::{debug, info};
use crate::{CloneButShorter, TestFrame, MotState};
use crate::marker_config_window::MarkersSettings;
use ats_usb::device::UsbDevice;
use ats_usb::packet::{CombinedMarkersReport, GeneralConfig, MarkerPattern, MotData, Packet};

pub fn transform_aimpoint_to_identity(center_aim: Point2<f64>, p1: Point2<f64>, p2: Point2<f64>, p3: Point2<f64>, p4: Point2<f64>) -> Option<Point2<f64>> {
    ats_cv::transform_aim_point(center_aim, p1, p2, p3, p4,
                        Point2::new(0.5, 1.), Point2::new(0., 0.5),
                        Point2::new(0.5, 0.), Point2::new(1., 0.5))
}

pub fn my_pnp(projections: &[Vector2<f64>]) -> Option<SQPSolution> {
    let _3dpoints = [
        Vector3::new((0.35 - 0.5) * 16./9., -0.5, 0.),
        Vector3::new((0.65 - 0.5) * 16./9., -0.5, 0.),
        Vector3::new((0.65 - 0.5) * 16./9., 0.5, 0.),
        Vector3::new((0.35 - 0.5) * 16./9., 0.5, 0.),
    ];
    let solver = sqpnp::PnpSolver::new(&_3dpoints, &projections, None, SolverParameters::default());
    if let Some(mut solver) = solver {
        solver.solve();
        debug!("pnp found {} solutions", solver.number_of_solutions());
        if solver.number_of_solutions() >= 1 {
            return Some(solver.solution_ptr(0).unwrap().clone());
        }
    } else {
        info!("pnp solver failed");
    }
    None
}

/// Given 4 points in the following shape
///
/// ```
/// +--x
/// |
/// y                 top
///
///
///   left                              right
///
///
///                  bottom
/// ```
///
/// Sort them into the order bottom, left, top, right
pub fn sort_diamond<T: Scalar + PartialOrd>(a: &mut [Point2<T>]) {
    if a[0].y < a[1].y { a.swap(0, 1); }
    if a[2].y > a[3].y { a.swap(2, 3); }
    if a[0].y < a[3].y { a.swap(0, 3); }
    if a[2].y > a[1].y { a.swap(2, 1); }
    if a[1].x > a[3].x { a.swap(1, 3); }
}

/// Given 4 points in the following shape
///
/// ```
/// +--x
/// |
/// y        a              b
///
///
///          c              d
/// ```
///
/// Sort them into the order a, b, d, c
pub fn sort_rectangle<T: Scalar + PartialOrd>(a: &mut [Point2<T>]) {
    if a[0].y > a[2].y { a.swap(0, 2); }
    if a[1].y > a[3].y { a.swap(1, 3); }
    if a[0].y > a[1].y { a.swap(0, 1); }
    if a[2].y > a[3].y { a.swap(2, 3); }
    if a[1].y > a[2].y { a.swap(1, 2); }
    if a[0].x > a[1].x { a.swap(0, 1); }
    if a[2].x < a[3].x { a.swap(2, 3); }
}

pub fn sort_points<T: Scalar + PartialOrd>(a: &mut [Point2<T>], pattern: MarkerPattern) {
    match pattern {
        MarkerPattern::Diamond => sort_diamond(a),
        MarkerPattern::Rectangle => sort_rectangle(a),
    }
}

pub struct MotRunner {
    pub state: MotState,
    pub device: Option<UsbDevice>,
    pub markers_settings: MarkersSettings,
    pub general_config: GeneralConfig,
    pub record_impact: bool,
    pub record_packets: bool,
    pub datapoints: Arc<Mutex<Vec<crate::TestFrame>>>,
    pub packets: Arc<Mutex<Vec<(u128, ats_usb::packet::PacketData)>>>,
    pub ui_update: RwSignal<()>,
    pub ui_ctx: Context,
    pub nf_offset: Vector2<f64>,
    pub wfnf_realign: bool,
}

pub async fn run(runner: Arc<Mutex<MotRunner>>) {
    tokio::join!(
        combined_markers_loop(runner.clone()),
        accel_stream(runner.clone()),
        impact_loop(runner.clone()),
    );
}

pub async fn frame_loop(runner: Arc<Mutex<MotRunner>>) {
    let device = runner.lock().device.c().unwrap();
    let mut mot_data_stream = device.stream_mot_data().await.unwrap();
    loop {
        if runner.lock().device.is_none() {
            return;
        }
        if let Some(mot_data) = mot_data_stream.next().await {
            let nf_data = mot_data.mot_data_nf;
            let wf_data = mot_data.mot_data_wf;
            let mut runner = runner.lock();
            let nf_data = ArrayVec::<MotData,16>::from_iter(nf_data.into_iter());
            // let nf_data = ArrayVec::<MotData,16>::from_iter(dummy_nf_data());
            let wf_data = ArrayVec::<MotData,16>::from_iter(wf_data.into_iter());

            let state = &mut runner.state;
            state.nf_data = Some(nf_data);
            state.wf_data = Some(wf_data);

            if runner.record_packets {
                runner.packets.lock().push((std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), ats_usb::packet::PacketData::ObjectReport(mot_data)));
            }
        }
    }
}

fn get_raycast_aimpoint(fv_state: &ats_cv::foveated::FoveatedAimpointState) -> (Matrix3<f32>, nalgebra::Point3<f32>, Option<Point2<f32>>) {
    let orientation = fv_state.filter.orientation;
    let position = fv_state.filter.position;

    let flip_yz = Matrix3::new(
        1., 0., 0.,
        0., -1., 0.,
        0., 0., -1.,
    );

    let rotmat = flip_yz * orientation.to_rotation_matrix() * flip_yz;
    let transmat = flip_yz * position;

    // 1920x1080 abe's wall
    let screen_height_meters = 1.2838;

    // 3840x2160 (16:9) SVT
    // let screen_height_meters: f64 = ???;

    let screen_3dpoints = ats_cv::calculate_screen_3dpoints(screen_height_meters, 16./9.);

    let fv_aimpoint = ats_cv::calculate_aimpoint_from_pose_and_screen_3dpoints(
        &rotmat,
        &transmat.coords,
        &screen_3dpoints,
    );

    (rotmat, transmat, fv_aimpoint)
}

async fn combined_markers_loop(runner: Arc<Mutex<MotRunner>>) {
    let device = runner.lock().device.c().unwrap();
    let mut combined_markers_stream = device.stream_combined_markers().await.unwrap();

    while runner.lock().device.is_some() {
        if let Some(combined_markers_report) = combined_markers_stream.next().await {
            let CombinedMarkersReport { nf_points, wf_points, nf_screen_ids, wf_screen_ids } = combined_markers_report;
            let mut runner = runner.lock();
            let nf_point_tuples = filter_and_create_point_tuples(&nf_points, &nf_screen_ids);
            let wf_point_tuples = filter_and_create_point_tuples(&wf_points, &wf_screen_ids);

            // println!("nf: {} wf: {}", filtered_nf_point_tuples.len(), filtered_wf_point_tuples.len());

            let nf_points_slice = nf_point_tuples.iter().map(|(_, _, p)| *p).collect::<Vec<_>>();
            let wf_points_slice = wf_point_tuples.iter().map(|(_, _, p)| *p).collect::<Vec<_>>();

            let nf_points_transformed = transform_points(&nf_points_slice, &runner.general_config.camera_model_nf);
            let wf_points_transformed = transform_points(&wf_points_slice, &runner.general_config.camera_model_wf);

            let nf_point_tuples = nf_point_tuples.iter().enumerate().map(|(i, (screen_id, id, _))| (*screen_id, *id, nf_points_transformed[i])).collect::<Vec<_>>();
            let wf_point_tuples = wf_point_tuples.iter().enumerate().map(|(i, (screen_id, id, _))| (*screen_id, *id, wf_points_transformed[i])).collect::<Vec<_>>();

            let wf_to_nf = ats_cv::wf_to_nf_points(&wf_points_transformed, &ats_cv::ros_opencv_intrinsics_type_convert(&runner.general_config.camera_model_nf), &ats_cv::ros_opencv_intrinsics_type_convert(&runner.general_config.camera_model_wf), runner.general_config.stereo_iso.cast());
            let wf_normalized: Vec<_> = wf_to_nf.iter().map(|&p| {
                let fx = runner.general_config.camera_model_nf.p.m11 as f64;
                let fy = runner.general_config.camera_model_nf.p.m22 as f64;
                let cx = runner.general_config.camera_model_nf.p.m13 as f64;
                let cy = runner.general_config.camera_model_nf.p.m23 as f64;
                Point2::new((p.x/4095.*98. - cx) / fx, (p.y/4095.*98. - cy) / fy)
            }).collect();
            let nf_normalized: Vec<_> = nf_points_transformed.iter().map(|&p| {
                let fx = runner.general_config.camera_model_nf.p.m11 as f64;
                let fy = runner.general_config.camera_model_nf.p.m22 as f64;
                let cx = runner.general_config.camera_model_nf.p.m13 as f64;
                let cy = runner.general_config.camera_model_nf.p.m23 as f64;
                Point2::new((p.x/4095.*98. - cx) / fx, (p.y/4095.*98. - cy) / fy)
            }).collect();

            let gravity_vec = runner.state.orientation.inverse_transform_vector(&Vector3::z_axis());
            let gravity_vec = UnitVector3::new_unchecked(gravity_vec.xzy());
            if runner.wfnf_realign {
                // Try to match widefield using brute force p3p, and then
                // using that to match nearfield
                if let Some((wf_match_ix, _)) = identify_markers2(&wf_normalized, gravity_vec.cast()) {
                    let wf_match = wf_match_ix.map(|i| wf_normalized[i].coords);
                    let (nf_match_ix, error) = match3(&nf_normalized, &wf_match);
                    if nf_match_ix.iter().all(Option::is_some) {
                        dbg!(error);
                        let nf_ordered = nf_match_ix.map(|i| nf_normalized[i.unwrap()].coords.push(1.0));
                        let wf_ordered = wf_match_ix.map(|i| wf_normalized[i].coords.push(1.0));
                        eprintln!("nf_ordered = {nf_ordered:?}");
                        eprintln!("wf_ordered = {wf_ordered:?}");
                        let q = calculate_rotational_offset(&wf_ordered, &nf_ordered);
                        runner.general_config.stereo_iso.rotation *= q.cast();
                        runner.wfnf_realign = false;
                    }
                }
            }

            // let nf_point_tuples_transformed = filtered_nf_point_tuples.iter().map(|(id, _)| *id).zip(&mut nf_points_transformed).collect::<Vec<_>>();
            // let wf_point_tuples_transformed = filtered_wf_point_tuples.iter().map(|(id, _)| *id).zip(&mut wf_points_transformed).collect::<Vec<_>>();

            // fn update_positions(pva2ds: &mut [Pva2d<f64>], points: Vec<(usize, &mut Point2<f64>)>) {
            //     for (i, point) in points {
            //         pva2ds[i].step();
            //         pva2ds[i].observe(point.coords.as_ref(), &[100.0, 100.0]);
            //         point.x = pva2ds[i].position()[0];
            //         point.y = pva2ds[i].position()[1];
            //     }
            // }

            // update_positions(&mut runner.state.nf_pva2ds, nf_point_tuples_transformed);
            // update_positions(&mut runner.state.wf_pva2ds, wf_point_tuples_transformed);

            // step at marker hz
            runner.state.fv_aimpoint_pva2d.step();

            runner.state.fv_state.observe_markers(&nf_normalized, &wf_normalized, gravity_vec.cast());

            let (rotmat, transmat, fv_aimpoint) = get_raycast_aimpoint(&runner.state.fv_state);

            runner.state.rotation_mat = rotmat.cast();
            runner.state.translation_mat = transmat.coords.cast();
            if let Some(fv_aimpoint) = fv_aimpoint {
                runner.state.fv_aimpoint = fv_aimpoint.cast();
            }

            if let Some(x) = calculate_individual_aimpoint(&nf_points_transformed, runner.state.orientation, None, &runner.general_config.camera_model_nf) {
                runner.state.nf_aimpoint = x;
            }

            if let Some(x) = calculate_individual_aimpoint(&wf_points_transformed, runner.state.orientation, Some(&runner.general_config.stereo_iso.cast()), &runner.general_config.camera_model_wf) {
                runner.state.wf_aimpoint = x;
            }

            let wf_markers = ats_cv::foveated::identify_markers2(&wf_normalized, gravity_vec.cast());
            // let nf_markers = ats_cv::foveated::identify_markers2(&nf_normalized, gravity_vec);
            // let nf_markers: ArrayVec<_, 16> = nf_markers.into_iter().flatten().collect();
            let wf_marker_ix: ArrayVec<_, 16> = match wf_markers {
                Some((markers, _)) => markers.into_iter().collect(),
                _ => Default::default(),
            };
            let wf_reproj: ArrayVec<_, 16> = match wf_markers {
                Some((_, reproj)) => reproj.map(|x| x.into()).into_iter().collect(),
                _ => Default::default(),
            };

            let mut nf_markers = ArrayVec::<_, 16>::new();

            if wf_marker_ix.len() >= 6 {
                let chosen_wf_markers: [_; 6] = [
                    wf_normalized[wf_marker_ix[0]].coords,
                    wf_normalized[wf_marker_ix[1]].coords,
                    wf_normalized[wf_marker_ix[2]].coords,
                    wf_normalized[wf_marker_ix[3]].coords,
                    wf_normalized[wf_marker_ix[4]].coords,
                    wf_normalized[wf_marker_ix[5]].coords,
                ];
                let match_result = ats_cv::foveated::match3(&nf_normalized, &chosen_wf_markers);
                for i in 0..6 {
                    let j = match_result.0[i];
                    if let Some(j) = j {
                        nf_markers.push(nf_points_transformed[j]);
                    } else {
                        nf_markers.push(Point2::new(-9999., -9999.));
                    }
                }
            }

            runner.state.nf_points = nf_point_tuples
                .into_iter()
                .filter(|p| !nf_markers.contains(&p.2))
                .collect();
            runner.state.wf_points = wf_point_tuples
                .iter()
                .enumerate()
                .filter(|(i, _)| !wf_marker_ix.contains(&i))
                .map(|x| *x.1)
                .collect();
            runner.state.nf_markers = nf_markers;
            runner.state.wf_markers = wf_marker_ix
                .into_iter()
                .map(|i| wf_points_transformed[i])
                .collect();
            runner.state.wf_reproj = wf_reproj;

            let index = runner.state.fv_aimpoint_history_index;
            runner.state.fv_aimpoint_history[index] = runner.state.nf_aimpoint;
            runner.state.fv_aimpoint_history_index = (index + 1) % runner.state.fv_aimpoint_history.len();

            if runner.record_packets {
                runner.packets.lock().push((std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), ats_usb::packet::PacketData::CombinedMarkersReport(combined_markers_report)));
            }
        }
    }
}

fn calculate_individual_aimpoint(points: &[Point2<f64>], orientation: Rotation3<f32>, iso: Option<&Isometry3<f32>>, intrinsics: &RosOpenCvIntrinsics<f32>) -> Option<Point2<f64>> {
    let fx = intrinsics.p.m11 * (4095./98.);
    let fy = intrinsics.p.m22 * (4095./98.);

    let gravity_vec = orientation.inverse_transform_vector(&Vector3::z());
	let gravity_angle = f64::atan2(-gravity_vec.z as f64, -gravity_vec.x as f64) + std::f64::consts::PI/2.;

    if points.len() > 3 {
        let mut rotated_points = ats_cv::mot_rotate(&points, -gravity_angle);
        sort_points(&mut rotated_points, MarkerPattern::Rectangle);
        // todo rotating back is bad, select with slice instead
        let points = ats_cv::mot_rotate(&rotated_points, gravity_angle);

        let projections = ats_cv::calculate_projections(
            &points,
            // 1/math.tan(38.3 / 180 * math.pi / 2) * 2047.5 (value used in the sim)
            Vector2::new(fx as f64, fy as f64),
            Vector2::new(4095., 4095.),
        );
        let solution = ats_cv::solve_pnp_with_dynamic_screen_points(
            projections.as_slice(),
            &[
                Point2::new(0.35, 0.),
                Point2::new(0.65, 0.),
                Point2::new(0.65, 1.),
                Point2::new(0.35, 1.),
            ],
            16./9.,
            1.,
        );
        if let Some(sol) = solution {
            let r_hat = Rotation3::from_matrix_unchecked(sol.r_hat.reshape_generic(Const::<3>, Const::<3>).transpose());
            let t = Translation3::from(sol.t);
            let tf = t * r_hat;
            let ctf = tf.inverse();

            let flip_yz = Matrix3::new(
                1.0, 0.0, 0.0,
                0.0, -1.0, 0.0,
                0.0, 0.0, -1.0,
            );

            if let Some(iso) = iso {
                let iso = iso.cast();
                let rotation_mat = flip_yz * (iso.rotation * ctf.rotation).to_rotation_matrix() * flip_yz;
                let translation_mat = flip_yz * ctf.translation.vector;

                let screen_3dpoints = ats_cv::calculate_screen_3dpoints(1., 16./9.);

                return ats_cv::calculate_aimpoint_from_pose_and_screen_3dpoints(&rotation_mat, &translation_mat, &screen_3dpoints);
            } else {
                let rotation_mat = flip_yz * ctf.rotation.matrix() * flip_yz;
                let translation_mat = flip_yz * ctf.translation.vector;

                let screen_3dpoints = ats_cv::calculate_screen_3dpoints(1., 16./9.);

                return ats_cv::calculate_aimpoint_from_pose_and_screen_3dpoints(&rotation_mat, &translation_mat, &screen_3dpoints);
            }
        }
    }
    None
}

fn filter_and_create_point_tuples(
    points: &[Point2<u16>],
    screen_ids: &[u8],
) -> Vec<(u8, u8, Point2<f64>)> {
    points
        .iter()
        .zip(screen_ids.iter())
        .enumerate()
        .filter_map(|(id, (pos, &screen_id))| {
            // screen id of 7 means there is no marker
            if screen_id < 7 && (400..3696).contains(&pos.x) && (400..3696).contains(&pos.y) {
                Some((screen_id, id as u8, Point2::new(pos.x as f64, pos.y as f64)))
            } else {
                None
            }
        })
        .collect()
}

fn transform_points(points: &[Point2<f64>], camera_intrinsics: &RosOpenCvIntrinsics<f32>) -> Vec<Point2<f64>> {
    let scaled_points = points.iter().map(|p| Point2::new(p.x / 4095. * 98., p.y / 4095. * 98.)).collect::<Vec<_>>();
    let undistorted_points = ats_cv::undistort_points(&ats_cv::ros_opencv_intrinsics_type_convert(camera_intrinsics), &scaled_points);
    undistorted_points.iter().map(|p| Point2::new(p.x / 98. * 4095., p.y / 98. * 4095.)).collect()
}

async fn accel_stream(runner: Arc<Mutex<MotRunner>>) {
    let device = runner.lock().device.c().unwrap();
    let mut accel_stream = device.stream_accel().await.unwrap();
    let mut prev_timestamp = None;
    while runner.lock().device.is_some() {
        if let Some(accel) = accel_stream.next().await {
            let mut runner = runner.lock();
            let accel_odr = runner.general_config.accel_odr;
            // println!("{:7.3?} {:7.3?}", accel.accel.xzy(), accel.gyro.xzy());
            // println!("{:7.3?}", accel.accel.norm());

            // print rotation in degrees
            // println!("Rotation: {}", accel.gyro.xzy().map(|x| x.to_degrees()));

            if let Some(_prev_timestamp) = prev_timestamp {
                if accel.timestamp < _prev_timestamp {
                    prev_timestamp = None;
                    continue;
                }
            }

            if let Some(prev_timestamp) = prev_timestamp {
                let elapsed = accel.timestamp as u64 - prev_timestamp as u64;
                // println!("elapsed: {}", elapsed);
                runner.state.fv_state.predict(-accel.accel.xzy(), -accel.gyro.xzy(), Duration::from_micros(elapsed));

                let sample_period = runner.state.madgwick.sample_period_mut();
                *sample_period = elapsed as f32/1_000_000.;
            } else {
                runner.state.fv_state.predict(-accel.accel.xzy(), -accel.gyro.xzy(), Duration::from_secs_f32(1./accel_odr as f32));
            }
            prev_timestamp = Some(accel.timestamp);

            let _ = runner.state.madgwick.update_imu(&Vector3::from(accel.gyro), &Vector3::from(accel.accel));
            runner.state.orientation = runner.state.madgwick.quat.to_rotation_matrix();

            ats_cv::series_add!(imu_data, (-accel.accel.xzy().cast(), -accel.gyro.xzy().cast()));

            let (rotmat, transmat, fv_aimpoint) = get_raycast_aimpoint(&runner.state.fv_state);

            runner.state.rotation_mat = rotmat.cast();
            runner.state.translation_mat = transmat.coords.cast();
            if let Some(fv_aimpoint) = fv_aimpoint {
                runner.state.fv_aimpoint = fv_aimpoint.cast();
            }

            if runner.record_packets {
                runner.packets.lock().push((std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), ats_usb::packet::PacketData::AccelReport(accel)));
            }
        }
    }
}

// todo use an aimpoint history to choose the aimpoint closest to the timestamp
async fn impact_loop(runner: Arc<Mutex<MotRunner>>) {
    let device = runner.lock().device.c().unwrap();
    let mut impact_stream = device.stream_impact().await.unwrap();
    while runner.lock().device.is_some() {
        if let Some(_impact) = impact_stream.next().await {
            let runner = runner.lock();
            if runner.record_impact {
                let mut frame = TestFrame {
                    fv_aimpoint_x: None,
                    fv_aimpoint_y: None,
                };

                {
                    let fv_aimpoint = runner.state.fv_aimpoint_history[runner.state.fv_aimpoint_history_index];
                    let fv_aimpoint = fv_aimpoint;
                    frame.fv_aimpoint_x = Some(fv_aimpoint.x);
                    frame.fv_aimpoint_y = Some(fv_aimpoint.y);
                }

                if runner.datapoints.is_locked() {
                    continue;
                }

                runner.datapoints.lock().push(frame);

                let ui_update = runner.ui_update.c();

                runner.ui_ctx.queue_main(move || {
                    leptos_reactive::SignalSet::set(&ui_update, ());
                });
            }
        }
    }
}

pub fn rescale(val: f64) -> f64 {
    rescale_generic(-2047.0, 2047.0, 0.0, 1.0, val)
}
fn rescale_generic(lo1: f64, hi1: f64, lo2: f64, hi2: f64, val: f64) -> f64 {
    (val - lo1) / (hi1 - lo1) * (hi2 - lo2) + lo2
}

fn _dummy_nf_data() -> [MotData; 4] {
    [
        MotData {
            cx: 1047,
            cy: 2047,
            area: 1,
            ..Default::default()
        },
        MotData {
            cx: 3347,
            cy: 2047,
            area: 1,
            ..Default::default()
        },
        MotData {
            cx: 2047,
            cy: 1047,
            area: 1,
            ..Default::default()
        },
        MotData {
            cx: 2047,
            cy: 3047,
            area: 1,
            ..Default::default()
        },
    ]
}
