use std::sync::Arc;
use ahrs::Ahrs;
use ats_cv::{calculate_rotational_offset, to_normalized_image_coordinates};
use ats_cv::foveated::{identify_markers2, match3};
use opencv_ros_camera::RosOpenCvIntrinsics;
use parking_lot::Mutex;
use std::time::{Duration, UNIX_EPOCH};
use arrayvec::ArrayVec;
use iui::concurrent::Context;
use leptos_reactive::RwSignal;
use nalgebra::{Const, Isometry3, Matrix3, Point2, Point3, Rotation3, Scalar, Translation3, UnitVector3, Vector2, Vector3};
use sqpnp::types::{SQPSolution, SolverParameters};
use tokio_stream::StreamExt;
use tracing::{debug, info};
use crate::{CloneButShorter, Marker, MotState, ScreenInfo, TestFrame};
use ats_usb::device::UsbDevice;
use ats_usb::packet::{CombinedMarkersReport, GeneralConfig, MotData};

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

pub fn sort_points<T: Scalar + PartialOrd>(a: &mut [Point2<T>]) {
    sort_rectangle(a);
}

pub struct MotRunner {
    pub state: MotState,
    pub device: Option<UsbDevice>,
    pub general_config: GeneralConfig,
    pub record_impact: bool,
    pub record_packets: bool,
    pub datapoints: Arc<Mutex<Vec<crate::TestFrame>>>,
    pub packets: Arc<Mutex<Vec<(u128, ats_usb::packet::PacketData)>>>,
    pub ui_update: RwSignal<()>,
    pub ui_ctx: Context,
    pub fv_offset: Vector2<f64>,
    pub wfnf_realign: bool,
    pub screen_info: ScreenInfo,
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

fn get_raycast_aimpoint(
    nearfield: &[ats_cv::foveated::Marker],
    widefield: &[ats_cv::foveated::Marker],
    fv_state: &ats_cv::foveated::FoveatedAimpointState,
    gravity: UnitVector3<f64>,
    screen_info: ScreenInfo,
) -> (Matrix3<f64>, nalgebra::Point3<f64>, Option<Point2<f64>>) {
    let match_ixs = fv_state.match_markers_from_eskf(nearfield, widefield, &screen_info.marker_points);
    let iso = ats_cv::foveated::do_pnp(match_ixs, nearfield, widefield, gravity, &screen_info.marker_points);

    let flip_yz = Matrix3::new(1., 0., 0., 0., -1., 0., 0., 0., -1.);

    // using the eskf as a fallback might make the aimpoint seem unstable in some scenarios when it
    // flips back and forth ¯\_(ツ)_/¯
    // let iso = iso.unwrap_or(Isometry3::from_parts(fv_state.filter.position.into(), fv_state.filter.orientation).cast());
    match iso {
        Some(iso) => {
            let orientation = iso.rotation;
            let position = iso.translation.vector;

            let flip_yz = Matrix3::new(1., 0., 0., 0., -1., 0., 0., 0., -1.);

            let rotmat = flip_yz * orientation.to_rotation_matrix() * flip_yz;
            let transmat = flip_yz * position;

            let screen_ratio =
                screen_info.screen_dimensions_meters[0] / screen_info.screen_dimensions_meters[1];
            let screen_3dpoints =
                ats_cv::calculate_screen_3dpoints(screen_info.screen_dimensions_meters[1], screen_ratio);

            let fv_aimpoint = ats_cv::calculate_aimpoint_from_pose_and_screen_3dpoints(
                &rotmat,
                &transmat,
                &screen_3dpoints,
            );

            let orientation = fv_state.filter.orientation;
            let position = fv_state.filter.position;

            let rotmat = flip_yz * orientation.to_rotation_matrix().cast() * flip_yz;
            let transmat = flip_yz * position.cast();

            (rotmat, transmat.into(), fv_aimpoint)
        }
        None => {
            let orientation = fv_state.filter.orientation;
            let position = fv_state.filter.position;

            let rotmat = flip_yz * orientation.to_rotation_matrix().cast() * flip_yz;
            let transmat = flip_yz * position.cast();

            (rotmat, transmat, None)
        }
    }
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

            let nf_points_slice = nf_point_tuples.iter().map(|(_, _, p)| *p).collect::<Vec<_>>();
            let wf_points_slice = wf_point_tuples.iter().map(|(_, _, p)| *p).collect::<Vec<_>>();

            let nf_points_transformed = transform_points(&nf_points_slice, &runner.general_config.camera_model_nf);
            let wf_points_transformed = transform_points(&wf_points_slice, &runner.general_config.camera_model_wf);

            let nf_point_tuples = nf_point_tuples.iter().enumerate().map(|(i, (screen_id, id, _))| (*screen_id, *id, nf_points_transformed[i])).collect::<Vec<_>>();
            let wf_point_tuples = wf_point_tuples.iter().enumerate().map(|(i, (screen_id, id, _))| (*screen_id, *id, wf_points_transformed[i])).collect::<Vec<_>>();

            let wf_normalized: ArrayVec<_, 16> = wf_points_transformed.iter().map(|&p| {
                to_normalized_image_coordinates(
                    p,
                    &ats_cv::ros_opencv_intrinsics_type_convert(&runner.general_config.camera_model_wf),
                    Some(&runner.general_config.stereo_iso.cast()),
                )
            }).collect();
            let nf_normalized: ArrayVec<_, 16> = nf_points_transformed.iter().map(|&p| {
                to_normalized_image_coordinates(
                    p,
                    &ats_cv::ros_opencv_intrinsics_type_convert(&runner.general_config.camera_model_nf),
                    None,
                )
            }).collect();
            runner.state.nf_markers2 = std::iter::zip(&nf_normalized, &nf_point_tuples)
                .map(|(&normalized, &(screen_id, mot_id, _))| Marker {
                    mot_id,
                    screen_id,
                    pattern_id: None,
                    normalized,
                })
                .collect();
            runner.state.wf_markers2 = std::iter::zip(&wf_normalized, &wf_point_tuples)
                .map(|(&normalized, &(screen_id, mot_id, _))| Marker {
                    mot_id,
                    screen_id,
                    pattern_id: None,
                    normalized,
                })
                .collect();

            let gravity_vec = runner.state.orientation.inverse_transform_vector(&Vector3::z_axis());
            let gravity_vec = UnitVector3::new_unchecked(gravity_vec.xzy());
            if runner.wfnf_realign {
                // Try to match widefield using brute force p3p, and then
                // using that to match nearfield
                if let Some((wf_match_ix, _, _)) = identify_markers2(&wf_normalized, None, gravity_vec.cast(), runner.screen_info.screen_dimensions_meters, runner.screen_info.marker_points) {
                    let wf_match = wf_match_ix.map(|i| wf_normalized[i].coords);
                    let (nf_match_ix, error) = match3(&nf_normalized, &wf_match);
                    if nf_match_ix.iter().all(Option::is_some) {
                        let nf_ordered = nf_match_ix.map(|i| nf_normalized[i.unwrap()].coords.push(1.0));
                        let wf_ordered = wf_match_ix.map(|i| wf_normalized[i].coords.push(1.0));
                        let q = calculate_rotational_offset(&wf_ordered, &nf_ordered);
                        runner.general_config.stereo_iso.rotation *= q.cast();
                        runner.wfnf_realign = false;
                    }
                }
            }

            // step at marker hz
            runner.state.fv_aimpoint_pva2d.step();

            let nf = &runner.state.nf_markers2.iter().map(|m| m.ats_cv_marker()).collect::<ArrayVec<_, 16>>();
            let wf = &runner.state.wf_markers2.iter().map(|m| m.ats_cv_marker()).collect::<ArrayVec<_, 16>>();
            let screen_info = runner.screen_info;
            runner.state.fv_state.observe_markers(nf, wf, gravity_vec.cast(), screen_info.screen_dimensions_meters, screen_info.marker_points);

            let (rotmat, transmat, fv_aimpoint) = get_raycast_aimpoint(nf, wf, &runner.state.fv_state, gravity_vec.cast(), runner.screen_info);

            runner.state.rotation_mat = rotmat.cast();
            runner.state.translation_mat = transmat.coords.cast();
            if let Some(fv_aimpoint) = fv_aimpoint {
                runner.state.fv_aimpoint_pva2d.observe(&[fv_aimpoint.x, fv_aimpoint.y], &[100., 100.]);
            }

            runner.state.fv_aimpoint = Point2::from(runner.state.fv_aimpoint_pva2d.position());

            let wf_markers = ats_cv::foveated::identify_markers2(&wf_normalized, None, gravity_vec.cast(), runner.screen_info.screen_dimensions_meters, runner.screen_info.marker_points);
            let (wf_marker_ix, wf_reproj): (ArrayVec<_, 16>, ArrayVec<_, 16>) = wf_markers
                .map(|(markers, reproj, _)| (
                    markers.into_iter().collect(),
                    reproj.map(|x| x.into()).into_iter().collect(),
                ))
                .unwrap_or_default();

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
                    runner.state.wf_markers2[wf_marker_ix[i]].pattern_id = Some(i as u8);
                    if let Some(j) = j {
                        nf_markers.push(nf_points_transformed[j]);
                        runner.state.nf_markers2[j].pattern_id = Some(i as u8);
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
            runner.state.fv_aimpoint_history[index] = runner.state.fv_aimpoint;
            runner.state.fv_aimpoint_history_index = (index + 1) % runner.state.fv_aimpoint_history.len();

            if runner.record_packets {
                runner.packets.lock().push((std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), ats_usb::packet::PacketData::CombinedMarkersReport(combined_markers_report)));
            }
        }
    }
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
            let accel_odr = runner.general_config.accel_config.accel_odr;
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

            let screen_info = runner.screen_info.clone();

            if let Some(prev_timestamp) = prev_timestamp {
                let elapsed = accel.timestamp as u64 - prev_timestamp as u64;
                // println!("elapsed: {}", elapsed);
                runner.state.fv_state.predict(-accel.accel.xzy(), -accel.gyro.xzy(), Duration::from_micros(elapsed), screen_info.screen_dimensions_meters);

                let sample_period = runner.state.madgwick.sample_period_mut();
                *sample_period = elapsed as f32/1_000_000.;
            } else {
                runner.state.fv_state.predict(-accel.accel.xzy(), -accel.gyro.xzy(), Duration::from_secs_f32(1./accel_odr as f32), screen_info.screen_dimensions_meters);
            }
            prev_timestamp = Some(accel.timestamp);

            let _ = runner.state.madgwick.update_imu(&Vector3::from(accel.gyro), &Vector3::from(accel.accel));
            runner.state.orientation = runner.state.madgwick.quat.to_rotation_matrix();

            ats_cv::series_add!(imu_data, (-accel.accel.xzy().cast(), -accel.gyro.xzy().cast()));

            // let (rotmat, transmat, fv_aimpoint) = get_raycast_aimpoint(&runner.state.fv_state, runner.screen_info);

            // runner.state.rotation_mat = rotmat.cast();
            // runner.state.translation_mat = transmat.coords.cast();
            // if let Some(fv_aimpoint) = fv_aimpoint {
            //     runner.state.fv_aimpoint = fv_aimpoint.cast();
            // }

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
            if runner.record_packets {
                runner.packets.lock().push((std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), ats_usb::packet::PacketData::ImpactReport(_impact)));
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
