use std::{sync::Arc, time::Duration};

use crate::{mot_runner::MotRunner, CloneButShorter};
use anyhow::Result;
use ats_usb::{
    device::UsbDevice,
    packets::vm::{AccelConfig, GeneralConfig, GyroConfig, Port},
};
use iui::{
    controls::{Button, Form},
    prelude::{Window, WindowType},
    UI,
};
use leptos_reactive::{
    create_effect, create_rw_signal, ReadSignal, RwSignal, SignalGet, SignalGetUntracked,
    SignalSet, SignalUpdate, SignalWith, SignalWithUntracked,
};
use opencv_ros_camera::RosOpenCvIntrinsics;
use parking_lot::Mutex;
use serialport::SerialPortInfo;
use serialport::SerialPortType::UsbPort;

pub fn config_window(
    ui: &UI,
    simulator_addr: Option<String>,
    udp_addr: Option<String>,
    mot_runner: Arc<Mutex<MotRunner>>,
    _tokio_handle: &tokio::runtime::Handle,
) -> (
    Window,
    ReadSignal<Option<UsbDevice>>,
    ReadSignal<AccelConfig>,
) {
    let ui_ctx = ui.async_context();
    let mut config_win = Window::new(&ui, "Config", 10, 10, WindowType::NoMenubar);
    config_win.on_closing(&ui, {
        let ui = ui.c();
        move |win: &mut Window| {
            win.hide(&ui);
        }
    });

    let device = create_rw_signal(None);
    let connected = move || device.with(|d| d.is_some());

    let (general_form, general_settings) =
        GeneralSettingsForm::new(&ui, device.read_only(), mot_runner, config_win.c());

    crate::layout! { &ui,
        let vbox = VerticalBox(padded: true) {
            Compact : let device_hbox = HorizontalBox(padded: true) {
                Stretchy : let device_combobox = Combobox() {}
                Compact : let refresh_button = Button("Refresh")
            }
            Compact : let tab_group = TabGroup() {} // sensor settings go in here
            Compact : let buttons_hbox = HorizontalBox(padded: true) {
                Compact : let apply_button = Button("Apply", enabled: connected)
                Compact : let save_button = Button("Save", enabled: connected)
                Compact : let reload_button = Button("Reload", enabled: connected)
                Compact : let load_defaults_button = Button("Load defaults", enabled: connected)
            }
        }
    }
    let (mut wf_form, wf_settings) = SensorSettingsForm::new(&ui, device.read_only(), Port::Wf);
    let (nf_form, nf_settings) = SensorSettingsForm::new(&ui, device.read_only(), Port::Nf);
    tab_group.append(&ui, "General", general_form);
    tab_group.append(&ui, "Wide field", wf_form.c());
    tab_group.append(&ui, "Near field", nf_form);
    tab_group.set_margined(&ui, 0, true);
    tab_group.set_margined(&ui, 1, true);
    tab_group.set_margined(&ui, 2, true);

    general_settings.device_pid.with(|pid| {
        match num_traits::FromPrimitive::from_u16(*pid) {
            Some(ats_usb::device::ProductId::PajUsb) | Some(ats_usb::device::ProductId::PajAts) => { wf_form.show(&ui); }
            Some(ats_usb::device::ProductId::PocAts) => { wf_form.hide(&ui); }
            _ => { wf_form.hide(&ui); }
        }
    });

    config_win.set_child(&ui, vbox);

    let device_list = create_rw_signal(Vec::<SerialPortInfo>::new());
    let device_combobox_on_selected = {
        let ui = ui.c();
        let config_win = config_win.c();
        let sim_addr = simulator_addr.c();
        let udp_addr = udp_addr.c();
        let general_settings = general_settings.c();
        move |i| {
            device.set(None);
            general_settings.clear();
            wf_settings.clear();
            nf_settings.clear();
            let Ok(i) = usize::try_from(i) else { return };
            let _device = device_list.with_untracked(|d| d.get(i).cloned());
            let sim_addr = sim_addr.c();
            let udp_addr = udp_addr.c();
            let general_settings = general_settings.c();
            let task = async move {
                let usb_device = if let Some(_device) = _device {
                    match &_device.port_type {
                        UsbPort(port_info) => Ok(UsbDevice::connect_serial(
                            &_device.port_name,
                            port_info.pid == ats_usb::device::ProductId::PajAts as u16,
                        )
                        .await?),
                        _ => Err(anyhow::anyhow!("Not a USB device")),
                    }
                } else if let Some(sim_addr) = sim_addr.as_ref() {
                    Ok(UsbDevice::connect_tcp(sim_addr)?)
                } else {
                    Ok(UsbDevice::connect_hub("0.0.0.0:0", udp_addr.as_ref().unwrap()).await?)
                };
                match usb_device {
                    Ok(usb_device) => {
                        general_settings.load_from_device(&usb_device, true).await?;
                        wf_settings.load_from_device(&usb_device).await?;
                        nf_settings.load_from_device(&usb_device).await?;
                        device.set(Some(usb_device));
                        Result::<()>::Ok(())
                    }
                    Err(e) => Err(e),
                }
            };
            ui.spawn({
                let ui = ui.c();
                let config_win = config_win.c();
                async move {
                    if let Err(e) = task.await {
                        config_win
                            .modal_err_async(&ui, "Failed to connect", &e.to_string())
                            .await;
                    }
                }
            });
        }
    };
    device_combobox.on_selected(&ui, device_combobox_on_selected.c());

    create_effect({
        // update device combobox when device_list changes
        let device_combobox = device_combobox.c();
        let ui = ui.c();
        let simulator_addr = simulator_addr.c();
        let udp_addr = udp_addr.c();
        move |_| {
            let mut device_combobox = device_combobox.c();
            device_combobox.clear(&ui);
            device_list.with(|device_list| {
                for device in device_list {
                    device_combobox.append(&ui, &display_for_serial_port(&device));
                }
            });
            if let Some(sim_addr) = &simulator_addr {
                device_combobox.append(&ui, &format!("Simulator @ {sim_addr}"));
            }
            if let Some(udp_addr) = &udp_addr {
                device_combobox.append(&ui, &format!("M4Hub @ {udp_addr}"));
            }
            device_combobox.enable(&ui);
        }
    });
    let mut refresh_device_list = {
        let config_win = config_win.c();
        let ui = ui.c();
        let simulator_addr = simulator_addr.c();
        let udp_addr = udp_addr.c();
        move || {
            let ports = serialport::available_ports();
            let ports: Vec<_> = match ports {
                Ok(p) => p,
                Err(e) => {
                    config_win.modal_err(&ui, "Failed to list serial ports", &e.to_string());
                    return;
                }
            }
            .into_iter()
            .filter(|port| {
                match &port.port_type {
                    UsbPort(port_info) => {
                        if port_info.vid == 0x1915
                            && (port_info.pid == 0x520F || port_info.pid == 0x5210)
                        {
                            if let Some(i) = port_info.interface {
                                // interface 0: cdc acm module
                                // interface 1: cdc acm module functional subordinate interface
                                // interface 2: cdc acm dfu
                                // interface 3: cdc acm dfu subordinate interface
                                i == 0
                            } else {
                                true
                            }
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            })
            .collect();
            device_list.set(ports.c());
            if simulator_addr.is_some() {
                device_combobox.set_selected(&ui, ports.len() as i32);
                device_combobox_on_selected(ports.len() as i32);
            } else if udp_addr.is_some() {
                device_combobox.set_selected(&ui, ports.len() as i32);
                device_combobox_on_selected(ports.len() as i32);
            } else if ports.len() > 0 {
                device_combobox.set_selected(&ui, 0);
                device_combobox_on_selected(0);
            } else {
                device_combobox_on_selected(-1);
            }
        }
    };
    refresh_device_list();
    refresh_button.on_clicked(&ui, move |_| refresh_device_list());

    let apply_button_on_click = {
        let config_win = config_win.c();
        let ui = ui.c();
        let general_settings = general_settings.c();
        move |device: UsbDevice| async move {
            let mut errors = vec![];
            general_settings.validate(&mut errors);
            if !errors.is_empty() {
                let mut message = String::new();
                for msg in &errors {
                    message.push_str(&msg);
                    message.push('\n');
                }
                config_win
                    .modal_err_async(&ui, "General Validation Error", &message)
                    .await;
                return false;
            }

            wf_settings.validate(&mut errors);
            if !errors.is_empty() {
                let mut message = String::new();
                for msg in &errors {
                    message.push_str(&msg);
                    message.push('\n');
                }
                config_win
                    .modal_err_async(&ui, "Wide Field Validation Error", &message)
                    .await;
                return false;
            }

            nf_settings.validate(&mut errors);
            if !errors.is_empty() {
                let mut message = String::new();
                for msg in &errors {
                    message.push_str(&msg);
                    message.push('\n');
                }
                config_win
                    .modal_err_async(&ui, "Near Field Validation Error", &message)
                    .await;
                return false;
            }

            if let Err(e) = general_settings.apply(&device).await {
                config_win
                    .modal_err_async(&ui, "Failed to apply general settings", &e.to_string())
                    .await;
                return false;
            };
            if let Err(e) = wf_settings.apply(&device).await {
                config_win
                    .modal_err_async(&ui, "Failed to apply wide field settings", &e.to_string())
                    .await;
                return false;
            };
            if let Err(e) = nf_settings.apply(&device).await {
                config_win
                    .modal_err_async(&ui, "Failed to apply near field settings", &e.to_string())
                    .await;
                return false;
            };
            return true;
        }
    };
    apply_button.on_clicked(&ui, {
        let f = apply_button_on_click.clone();
        let device = device.c();
        let ui = ui.c();
        move |apply_button| {
            let Some(device) = device.get_untracked() else {
                return;
            };
            let f = f.clone();
            let mut apply_button = apply_button.c();
            let ui2 = ui.c();
            ui.spawn(async move {
                f(device).await;
                apply_button.set_text(&ui2, "Applied!");
                tokio::time::sleep(Duration::from_secs(3)).await;
                apply_button.set_text(&ui2, "Apply");
            });
        }
    });
    save_button.on_clicked(&ui, {
        let config_win = config_win.c();
        let ui = ui.c();
        let device = device.c();
        move |save_button| {
            let Some(device) = device.get_untracked() else {
                return;
            };
            ui.spawn({
                let ui = ui.c();
                let config_win = config_win.c();
                let apply_button_on_click = apply_button_on_click.c();
                let mut save_button = save_button.c();
                async move {
                    if !apply_button_on_click(device.clone()).await {
                        // callback should have already displayed an error modal, just return
                        return;
                    }
                    if let Err(e) = device.flash_settings().await {
                        config_win
                            .modal_err_async(
                                &ui,
                                "Failed to request flash settings",
                                &e.to_string(),
                            )
                            .await;
                    }
                    save_button.set_text(&ui, "Saved!");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    save_button.set_text(&ui, "Save");
                }
            })
        }
    });
    load_defaults_button.on_clicked(&ui, {
        let general_settings = general_settings.c();
        move |_| {
            if let Some(_device) = device.get_untracked() {
                general_settings.load_defaults();
                nf_settings.load_defaults();
                wf_settings.load_defaults();
            }
        }
    });

    reload_button.on_clicked(&ui, {
        let general_settings = general_settings.c();
        move |_| {
            if let Some(device) = device.get_untracked() {
                let general_settings = general_settings.c();
                ui_ctx.spawn(async move {
                    _ = general_settings.load_from_device(&device, false).await;
                    _ = nf_settings.load_from_device(&device).await;
                    _ = wf_settings.load_from_device(&device).await;
                });
            }
        }
    });

    (
        config_win,
        device.read_only(),
        general_settings.accel_config.read_only(),
    )
}

#[derive(Clone)]
struct GeneralSettingsForm {
    device_uuid: RwSignal<[u8; 6]>,
    device_pid: RwSignal<u16>,
    impact_threshold: RwSignal<i32>,
    accel_config: RwSignal<AccelConfig>,
    gyro_config: RwSignal<GyroConfig>,
    nf_intrinsics: RwSignal<RosOpenCvIntrinsics<f32>>,
    wf_intrinsics: RwSignal<RosOpenCvIntrinsics<f32>>,
    stereo_iso: RwSignal<nalgebra::Isometry3<f32>>,
    mot_runner: Arc<Mutex<MotRunner>>,
}

impl GeneralSettingsForm {
    fn new(
        ui: &UI,
        device: ReadSignal<Option<UsbDevice>>,
        mot_runner: Arc<Mutex<MotRunner>>,
        win: Window,
    ) -> (Form, Self) {
        let device_uuid = create_rw_signal([0; 6]);
        let device_pid = create_rw_signal(0u16);
        let connected = move || device.with(|d| d.is_some());
        let impact_threshold = create_rw_signal(0);
        let accel_config = create_rw_signal(AccelConfig::default());
        let gyro_config = create_rw_signal(GyroConfig::default());
        let nf_intrinsics =
            create_rw_signal(RosOpenCvIntrinsics::from_params(145., 0., 145., 45., 45.));
        let wf_intrinsics =
            create_rw_signal(RosOpenCvIntrinsics::from_params(34., 0., 34., 45., 45.));
        let stereo_iso = create_rw_signal(nalgebra::Isometry3::identity());
        crate::layout! { &ui,
            let form = Form(padded: true) {
                (Compact, "Device UUID") : let x = Label(move || {
                    if connected() {
                        let id = device_uuid.get();
                        format!(
                            "{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                            id[0],
                            id[1],
                            id[2],
                            id[3],
                            id[4],
                            id[5],
                        )
                    } else {
                        "".into()
                    }
                })
                (Compact, "Impact threshold") : let x = Spinbox(enabled: connected, signal: impact_threshold)
                (Compact, "Accelerometer Config") : let x = HorizontalBox(padded: true) {
                    Compact : let upload_accel_config = Button("Upload")
                    Compact : let download_accel_config = Button("Download")
                }
                (Compact, "Gyroscope Config") : let x = HorizontalBox(padded: true) {
                    Compact : let upload_gyro_config = Button("Upload")
                    Compact : let download_gyro_config = Button("Download")
                }
                (Compact, "Nearfield Calibration") : let x = HorizontalBox(padded: true) {
                    Compact : let upload_nf_json = Button("Upload")
                    Compact : let download_nf_json = Button("Download")
                }
                (Compact, "Widefield Calibration") : let x = HorizontalBox(padded: true) {
                    Compact : let upload_wf_json = Button("Upload")
                    Compact : let download_wf_json = Button("Download")
                }
                (Compact, "Stereo Calibration") : let x = HorizontalBox(padded: true) {
                    Compact : let upload_stereo_json = Button("Upload")
                    Compact : let download_stereo_json = Button("Download")
                    Compact : let sync_stereo = Button("Sync")
                }
            }
        }
        set_calibration_upload_handlers(
            &ui,
            &mut upload_nf_json,
            &mut upload_wf_json,
            &mut upload_stereo_json,
            nf_intrinsics.c(),
            wf_intrinsics.c(),
            stereo_iso.c(),
            win.c(),
        );
        set_accel_upload_handler(&ui, &mut upload_accel_config, accel_config.c(), win.c());
        set_gyro_upload_handler(&ui, &mut upload_gyro_config, gyro_config.c(), win.c());
        set_calibration_download_handlers(
            &ui,
            &mut download_nf_json,
            &mut download_wf_json,
            &mut download_stereo_json,
            nf_intrinsics.c(),
            wf_intrinsics.c(),
            stereo_iso.c(),
            win.c(),
        );
        set_accel_download_handler(&ui, &mut download_accel_config, accel_config.c(), win.c());
        set_gyro_download_handler(&ui, &mut download_gyro_config, gyro_config.c(), win.c());

        sync_stereo.on_clicked(&ui, {
            let stereo_iso = stereo_iso.c();
            let mot_runner = mot_runner.c();
            move |_| {
                let iso = mot_runner.lock().general_config.stereo_iso;
                stereo_iso.set(iso);
            }
        });

        (
            form,
            Self {
                impact_threshold,
                accel_config,
                gyro_config,
                nf_intrinsics,
                wf_intrinsics,
                stereo_iso,
                mot_runner,
                device_uuid,
                device_pid,
            },
        )
    }

    async fn load_from_device(&self, device: &UsbDevice, first_load: bool) -> Result<()> {
        let timeout = Duration::from_millis(5000);
        let config = retry(|| device.read_config(), timeout, 3).await.unwrap()?;
        let props = retry(|| device.read_props(), timeout, 3).await.unwrap()?;

        self.impact_threshold
            .set(i32::from(config.impact_threshold));
        self.accel_config.set(config.accel_config);
        self.gyro_config.set(config.gyro_config);
        self.nf_intrinsics.set(config.camera_model_nf.clone());
        self.wf_intrinsics.set(config.camera_model_wf.clone());
        self.stereo_iso.set(config.stereo_iso.clone());
        self.device_uuid.set(props.uuid);
        self.device_pid.set(props.product_id);

        if first_load {
            self.mot_runner.lock().general_config = config;
        }
        Ok(())
    }

    fn validate(&self, errors: &mut Vec<String>) {
        macro_rules! validators {
            ($($display:literal $reg:ident : $ty:ty $({ $( $check:expr ),* $(,)? })? ),* $(,)?) => {
                $(
                    #[allow(unused_variables)]
                    let $reg = self.$reg.with_untracked(|s| s.parse::<$ty>());
                    match &$reg {
                        Ok(_) => (),
                        Err(e) => errors.push(format!("{}: {}", $display, e)),
                    }
                )*
                if !errors.is_empty() {
                    return
                }
                $(
                    // SAFETY: all the values are already checked for errors. Can convert to
                    // unwrap_unchecked if necessary.
                    #[allow(unused_variables)]
                    let $reg = $reg.unwrap();
                )*
                $(
                    $($(
                        let (test_result, msg) = ($check)($reg);
                        if !test_result {
                            errors.push(format!("{}: {}", $display, msg));
                        }
                    )*)?
                )*
            }
        }
        validators! {}
        if !(0..256).contains(&self.impact_threshold.get_untracked()) {
            errors.push("impact threshold: must be between 0 and 255".into());
        }
    }

    /// Make sure to call `validate()` before calling this method.
    async fn apply(&self, device: &UsbDevice) -> Result<()> {
        let config = GeneralConfig {
            impact_threshold: self.impact_threshold.get_untracked() as u8,
            accel_config: self.accel_config.get_untracked(),
            gyro_config: self.gyro_config.get_untracked(),
            camera_model_nf: self.nf_intrinsics.get_untracked(),
            camera_model_wf: self.wf_intrinsics.get_untracked(),
            stereo_iso: self.stereo_iso.get_untracked(),
        };
        device.write_config(config.clone()).await?;
        {
            let general_config = &mut self.mot_runner.lock().general_config;
            general_config.impact_threshold = config.impact_threshold;
            general_config.accel_config = config.accel_config;
            general_config.gyro_config = config.gyro_config;
            general_config.camera_model_nf = config.camera_model_nf;
            general_config.camera_model_wf = config.camera_model_wf;
            general_config.stereo_iso = config.stereo_iso;
        }
        Ok(())
    }

    fn clear(&self) {
        self.accel_config.set(AccelConfig::default());
        self.gyro_config.set(GyroConfig::default());
    }

    fn load_defaults(&self) {
        self.impact_threshold.set(5);
        self.accel_config.set(AccelConfig::default());
        self.gyro_config.set(GyroConfig::default());
        self.nf_intrinsics
            .set(RosOpenCvIntrinsics::from_params(145., 0., 145., 45., 45.));
        self.wf_intrinsics
            .set(RosOpenCvIntrinsics::from_params(34., 0., 34., 45., 45.));
        self.stereo_iso.set(nalgebra::Isometry3::identity());
    }
}

fn set_calibration_upload_handlers(
    ui: &UI,
    upload_nf: &mut Button,
    upload_wf: &mut Button,
    upload_stereo: &mut Button,
    nf_intrinsics: RwSignal<RosOpenCvIntrinsics<f32>>,
    wf_intrinsics: RwSignal<RosOpenCvIntrinsics<f32>>,
    stereo_iso: RwSignal<nalgebra::Isometry3<f32>>,
    win: Window,
) {
    upload_nf.on_clicked(&ui, {
        let ui = ui.c();
        let win = win.c();
        move |_| {
            if let Some(path) = win.open_file(&ui) {
                let Ok(()) = (|| {
                    let reader = std::fs::File::open(&path)?;
                    let intrinsics =
                        ats_common::get_intrinsics_from_opencv_camera_calibration_json(reader)?;
                    nf_intrinsics.set(intrinsics);
                    win.modal_msg(
                        &ui,
                        "Uploaded calibration",
                        "Successfully uploaded calibration",
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })() else {
                    win.modal_err(&ui, "Failed to upload calibration", "Failed to read file");
                    return;
                };
            } else {
                win.modal_err(&ui, "Failed to upload calibration", "No file selected");
            }
        }
    });

    upload_wf.on_clicked(&ui, {
        let ui = ui.c();
        let win = win.c();
        move |_| {
            if let Some(path) = win.open_file(&ui) {
                let Ok(()) = (|| {
                    let reader = std::fs::File::open(&path)?;
                    let intrinsics =
                        ats_common::get_intrinsics_from_opencv_camera_calibration_json(reader)?;
                    wf_intrinsics.set(intrinsics);
                    win.modal_msg(
                        &ui,
                        "Uploaded calibration",
                        "Successfully uploaded calibration",
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })() else {
                    win.modal_err(&ui, "Failed to upload calibration", "Failed to read file");
                    return;
                };
            } else {
                win.modal_err(&ui, "Failed to upload calibration", "No file selected");
            }
        }
    });

    upload_stereo.on_clicked(&ui, {
        let ui = ui.c();
        let win = win.c();
        move |_| {
            if let Some(path) = win.open_file(&ui) {
                let Ok(()) = (|| {
                    let reader = std::fs::File::open(&path)?;
                    let iso = ats_common::get_isometry_from_opencv_stereo_calibration_json(reader)?;
                    stereo_iso.set(iso);
                    win.modal_msg(
                        &ui,
                        "Uploaded calibration",
                        "Successfully uploaded calibration",
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })() else {
                    win.modal_err(&ui, "Failed to upload calibration", "Failed to read file");
                    return;
                };
            } else {
                win.modal_err(&ui, "Failed to upload calibration", "No file selected");
            }
        }
    });
}

fn set_calibration_download_handlers(
    ui: &UI,
    download_nf: &mut Button,
    download_wf: &mut Button,
    download_stereo: &mut Button,
    nf_intrinsics: RwSignal<RosOpenCvIntrinsics<f32>>,
    wf_intrinsics: RwSignal<RosOpenCvIntrinsics<f32>>,
    stereo_iso: RwSignal<nalgebra::Isometry3<f32>>,
    win: Window,
) {
    download_nf.on_clicked(&ui, {
        let ui = ui.c();
        let win = win.c();
        move |_| {
            if let Some(path) = win.save_file(&ui) {
                let Ok(()) = (|| {
                    let writer = std::fs::File::create(&path)?;
                    ats_common::write_opencv_minimal_camera_calibration_json(
                        &nf_intrinsics.get(),
                        writer,
                    )?;
                    win.modal_msg(
                        &ui,
                        "Downloaded calibration",
                        "Successfully downloaded calibration",
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })() else {
                    win.modal_err(
                        &ui,
                        "Failed to download calibration",
                        "Failed to write file",
                    );
                    return;
                };
            } else {
                win.modal_err(&ui, "Failed to download calibration", "No file selected");
            }
        }
    });

    download_wf.on_clicked(&ui, {
        let ui = ui.c();
        let win = win.c();
        move |_| {
            if let Some(path) = win.save_file(&ui) {
                let Ok(()) = (|| {
                    let writer = std::fs::File::create(&path)?;
                    ats_common::write_opencv_minimal_camera_calibration_json(
                        &wf_intrinsics.get(),
                        writer,
                    )?;
                    win.modal_msg(
                        &ui,
                        "Downloaded calibration",
                        "Successfully downloaded calibration",
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })() else {
                    win.modal_err(
                        &ui,
                        "Failed to download calibration",
                        "Failed to write file",
                    );
                    return;
                };
            } else {
                win.modal_err(&ui, "Failed to download calibration", "No file selected");
            }
        }
    });

    download_stereo.on_clicked(&ui, {
        let ui = ui.c();
        let win = win.c();
        move |_| {
            if let Some(path) = win.save_file(&ui) {
                let Ok(()) = (|| {
                    let writer = std::fs::File::create(&path)?;
                    ats_common::write_opencv_minimal_stereo_calibration_json(
                        &stereo_iso.get(),
                        writer,
                    )?;
                    win.modal_msg(
                        &ui,
                        "Downloaded calibration",
                        "Successfully downloaded calibration",
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })() else {
                    win.modal_err(
                        &ui,
                        "Failed to download calibration",
                        "Failed to write file",
                    );
                    return;
                };
            } else {
                win.modal_err(&ui, "Failed to download calibration", "No file selected");
            }
        }
    });
}

fn set_accel_upload_handler(
    ui: &UI,
    upload_accel: &mut Button,
    accel_config_signal: RwSignal<AccelConfig>,
    win: Window,
) {
    upload_accel.on_clicked(&ui, {
        let ui = ui.c();
        let win = win.c();
        move |_| {
            if let Some(path) = win.open_file(&ui) {
                let Ok(()) = (|| {
                    let reader = std::fs::File::open(&path)?;
                    let accel_config: AccelConfig = serde_json::from_reader(reader)?;
                    accel_config_signal.set(accel_config);
                    win.modal_msg(
                        &ui,
                        "Uploaded configuration",
                        "Successfully uploaded configuration",
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })() else {
                    win.modal_err(&ui, "Failed to upload configuration", "Failed to read file");
                    return;
                };
            } else {
                win.modal_err(&ui, "Failed to upload configuration", "No file selected");
            }
        }
    });
}

fn set_accel_download_handler(
    ui: &UI,
    download_accel: &mut Button,
    accel_config_signal: RwSignal<AccelConfig>,
    win: Window,
) {
    download_accel.on_clicked(&ui, {
        let ui = ui.c();
        let win = win.c();
        move |_| {
            if let Some(path) = win.save_file(&ui) {
                let Ok(()) = (|| {
                    let writer = std::fs::File::create(&path)?;
                    serde_json::to_writer(writer, &accel_config_signal.get())?;
                    win.modal_msg(
                        &ui,
                        "Downloaded configuration",
                        "Successfully downloaded configuration",
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })() else {
                    win.modal_err(
                        &ui,
                        "Failed to download configuration",
                        "Failed to write file",
                    );
                    return;
                };
            } else {
                win.modal_err(&ui, "Failed to download configuration", "No file selected");
            }
        }
    });
}

fn set_gyro_upload_handler(
    ui: &UI,
    upload_gyro: &mut Button,
    gyro_config_signal: RwSignal<GyroConfig>,
    win: Window,
) {
    upload_gyro.on_clicked(&ui, {
        let ui = ui.c();
        let win = win.c();
        move |_| {
            if let Some(path) = win.open_file(&ui) {
                let Ok(()) = (|| {
                    let reader = std::fs::File::open(&path)?;
                    let gyro_config: GyroConfig = serde_json::from_reader(reader)?;
                    gyro_config_signal.set(gyro_config);
                    win.modal_msg(
                        &ui,
                        "Uploaded configuration",
                        "Successfully uploaded configuration",
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })() else {
                    win.modal_err(&ui, "Failed to upload configuration", "Failed to read file");
                    return;
                };
            } else {
                win.modal_err(&ui, "Failed to upload configuration", "No file selected");
            }
        }
    });
}

fn set_gyro_download_handler(
    ui: &UI,
    download_gyro: &mut Button,
    gyro_config_signal: RwSignal<GyroConfig>,
    win: Window,
) {
    download_gyro.on_clicked(&ui, {
        let ui = ui.c();
        let win = win.c();
        move |_| {
            if let Some(path) = win.save_file(&ui) {
                let Ok(()) = (|| {
                    let writer = std::fs::File::create(&path)?;
                    serde_json::to_writer(writer, &gyro_config_signal.get())?;
                    win.modal_msg(
                        &ui,
                        "Downloaded configuration",
                        "Successfully downloaded configuration",
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })() else {
                    win.modal_err(
                        &ui,
                        "Failed to download configuration",
                        "Failed to write file",
                    );
                    return;
                };
            } else {
                win.modal_err(&ui, "Failed to download configuration", "No file selected");
            }
        }
    });
}

#[derive(Copy, Clone)]
struct SensorSettingsForm {
    port: Port,
    pid: RwSignal<String>,
    resolution_x: RwSignal<String>,
    resolution_y: RwSignal<String>,
    exposure_time: RwSignal<String>,
    frame_period: RwSignal<String>,
    brightness_threshold: RwSignal<String>,
    noise_threshold: RwSignal<String>,
    area_threshold_min: RwSignal<String>,
    area_threshold_max: RwSignal<String>,
    max_object_cnt: RwSignal<String>,

    operation_mode: RwSignal<i32>,
    frame_subtraction: RwSignal<i32>,
    gain: RwSignal<i32>,
}

impl SensorSettingsForm {
    fn new(ui: &UI, device: ReadSignal<Option<UsbDevice>>, port: Port) -> (Form, Self) {
        let connected = move || device.with(|d| d.is_some());
        let pid = create_rw_signal(String::new());
        let resolution_x = create_rw_signal(String::new());
        let resolution_y = create_rw_signal(String::new());
        let exposure_time = create_rw_signal(String::new());
        let frame_period = create_rw_signal(String::new());
        let brightness_threshold = create_rw_signal(String::new());
        let noise_threshold = create_rw_signal(String::new());
        let area_threshold_min = create_rw_signal(String::new());
        let area_threshold_max = create_rw_signal(String::new());
        let max_object_cnt = create_rw_signal(String::new());
        let operation_mode = create_rw_signal(0);
        let frame_subtraction = create_rw_signal(0);
        let gain = create_rw_signal(0);

        let exposure_time_ms = move || match exposure_time.with(|s| s.parse::<u16>()) {
            Ok(n) => format!("{:.4}", f64::from(n) * 200.0 / 1e6),
            Err(_) => format!("???"),
        };
        let frame_period_ms = move || match frame_period.with(|s| s.parse::<u32>()) {
            Ok(n) => format!("{:.4}", f64::from(n) / 1e4),
            Err(_) => format!("???"),
        };
        let fps = move || match frame_period.with(|s| s.parse::<u32>()) {
            Ok(n) => format!("{:.2}", 1e7 / f64::from(n)),
            Err(_) => format!("???"),
        };
        crate::layout! { &ui,
            let form = Form(padded: true) {
                (Compact, "Product ID")               : let product_id = Entry(value: pid, enabled: false)
                (Compact, "DSP brightness threshold") : let x = Entry(enabled: connected, signal: brightness_threshold)
                (Compact, "DSP noise threshold")      : let x = Entry(enabled: connected, signal: noise_threshold)
                (Compact, "DSP area threshold min")   : let x = Entry(enabled: connected, signal: area_threshold_min)
                (Compact, "DSP area threshold max")   : let x = Entry(enabled: connected, signal: area_threshold_max)
                (Compact, "DSP maximum object count") : let x = Entry(enabled: connected, signal: max_object_cnt)
                (Compact, "DSP operation mode")       : let x = Combobox(enabled: connected, signal: operation_mode) { "Normal", "Tracking" }
                (Compact, "Exposure time") : let x = HorizontalBox(padded: true) {
                    Stretchy : let e = Entry(
                        enabled: connected,
                        signal: exposure_time,
                    )
                    Compact : let expo_label = LayoutGrid() {
                        (0, 0)(1, 1) Vertical (Start, Center) : let s = Label(move || format!("× 200ns = {} ms", exposure_time_ms()))
                    }
                }
                (Compact, "Frame period") : let x = HorizontalBox(padded: true) {
                    Stretchy : let e = Entry(
                        enabled: connected,
                        signal: frame_period,
                    )
                    Compact : let fps_label = LayoutGrid() {
                        (0, 0)(1, 1) Vertical (Start, Center) : let s = Label(move || format!("{} ms ({} fps)", frame_period_ms(), fps()))
                    }
                }
                (Compact, "Frame subtraction")  : let x = Combobox(enabled: connected, signal: frame_subtraction) { "Off", "On" }
                (Compact, "Gain")               : let gain_combobox = Combobox(enabled: connected, signal: gain) {}
                (Compact, "Scale resolution X") : let x = Entry(enabled: connected, signal: resolution_x)
                (Compact, "Scale resolution Y") : let x = Entry(enabled: connected, signal: resolution_y)
            }
        }
        for (label, _) in &GAIN_TABLE {
            gain_combobox.append(&ui, label);
        }
        (
            form,
            Self {
                port,
                pid,
                resolution_x,
                resolution_y,
                exposure_time,
                frame_period,
                brightness_threshold,
                noise_threshold,
                area_threshold_min,
                area_threshold_max,
                max_object_cnt,
                operation_mode,
                frame_subtraction,
                gain,
            },
        )
    }

    async fn load_from_device(&self, device: &UsbDevice) -> Result<()> {
        self.pid.set("Connecting...".into());
        let timeout = Duration::from_millis(2000);
        let pid = retry(|| device.product_id(self.port), timeout, 3)
            .await
            .unwrap()?;
        let res_x = retry(|| device.resolution_x(self.port), timeout, 3)
            .await
            .unwrap()?;
        let res_y = retry(|| device.resolution_y(self.port), timeout, 3)
            .await
            .unwrap()?;
        let expo = retry(|| device.exposure_time(self.port), timeout, 3)
            .await
            .unwrap()?;
        let frame_period = retry(|| device.frame_period(self.port), timeout, 3)
            .await
            .unwrap()?;
        let brightness_threshold = retry(|| device.brightness_threshold(self.port), timeout, 3)
            .await
            .unwrap()?;
        let noise_threshold = retry(|| device.noise_threshold(self.port), timeout, 3)
            .await
            .unwrap()?;
        let area_threshold_min = retry(|| device.area_threshold_min(self.port), timeout, 3)
            .await
            .unwrap()?;
        let area_threshold_max = device.area_threshold_max(self.port).await?;
        let max_object_cnt = retry(|| device.max_object_cnt(self.port), timeout, 3)
            .await
            .unwrap()?;
        let operation_mode = retry(|| device.operation_mode(self.port), timeout, 3)
            .await
            .unwrap()?;
        let frame_subtraction = retry(|| device.frame_subtraction(self.port), timeout, 3)
            .await
            .unwrap()?;
        let gain_1 = retry(|| device.gain_1(self.port), timeout, 3)
            .await
            .unwrap()?;
        let gain_2 = retry(|| device.gain_2(self.port), timeout, 3)
            .await
            .unwrap()?;

        self.pid.set(format!("0x{pid:04x}"));
        self.resolution_x.set(res_x.to_string());
        self.resolution_y.set(res_y.to_string());
        self.exposure_time.set(expo.to_string());
        self.frame_period.set(frame_period.to_string());
        self.brightness_threshold
            .set(brightness_threshold.to_string());
        self.noise_threshold.set(noise_threshold.to_string());
        self.area_threshold_min.set(area_threshold_min.to_string());
        self.area_threshold_max.set(area_threshold_max.to_string());
        self.max_object_cnt.set(max_object_cnt.to_string());

        self.operation_mode.set(i32::from(operation_mode));
        self.frame_subtraction.set(i32::from(frame_subtraction));
        self.gain
            .set(i32::from(Gain::index_from_reg(gain_1, gain_2)));
        Ok(())
    }

    fn validate(&self, errors: &mut Vec<String>) {
        macro_rules! validators {
            ($($display:literal $reg:ident : $ty:ty $({ $( $check:expr ),* $(,)? })? ),* $(,)?) => {
                $(
                    #[allow(unused_variables)]
                    let $reg = self.$reg.with_untracked(|s| s.parse::<$ty>());
                    match &$reg {
                        Ok(_) => (),
                        Err(e) => errors.push(format!("{}: {}", $display, e)),
                    }
                )*
                if !errors.is_empty() {
                    return
                }
                $(
                    // SAFETY: all the values are already checked for errors. Can convert to
                    // unwrap_unchecked if necessary.
                    #[allow(unused_variables)]
                    let $reg = $reg.unwrap();
                )*
                $(
                    $($(
                        let (test_result, msg) = ($check)($reg);
                        if !test_result {
                            errors.push(format!("{}: {}", $display, msg));
                        }
                    )*)?
                )*
            }
        }
        validators! {
            "exposure time" exposure_time: u16 {
                |x| (x >= 100, "must be >= 20 µs"),
                |x| ((200..=i64::from(frame_period) - 27000).contains(&(i64::from(x)*2)), "must be between 20 µs and frame period − 2.7 ms"),
            },
            "frame period" frame_period: u32 { |x| (x >= 49780, "must be >= 4.978 ms") },
            "brightness threshold" brightness_threshold: u8,
            "noise threshold" noise_threshold: u8,
            "area threshold min" area_threshold_min: u8,
            "area threshold max" area_threshold_max: u16 { |x| (x < (1 << 14), "must be < 16384") },
            "max object count" max_object_cnt: u8 { |x| ((1..=16).contains(&x), "must be between 1 and 16") },
            "scale resolution X" resolution_x: u16 { |x| ((1..=4095).contains(&x), "must be between 1 and 4095") },
            "scale resolution Y" resolution_y: u16 { |x| ((1..=4095).contains(&x), "must be between 1 and 4095") },
        }
    }

    /// Make sure to call `validate()` before calling this method.
    async fn apply(&self, device: &UsbDevice) -> Result<()> {
        let gain = usize::try_from(self.gain.get_untracked()).unwrap();
        let gain = GAIN_TABLE[gain].1;

        tokio::try_join!(
            device.set_resolution_x(
                self.port,
                self.resolution_x.with_untracked(|v| v.parse().unwrap())
            ),
            device.set_resolution_y(
                self.port,
                self.resolution_y.with_untracked(|v| v.parse().unwrap())
            ),
            device.set_gain_1(self.port, gain.b_global),
            device.set_gain_2(self.port, gain.b_ggh),
            device.set_exposure_time(
                self.port,
                self.exposure_time.with_untracked(|v| v.parse().unwrap())
            ),
            device.set_brightness_threshold(
                self.port,
                self.brightness_threshold
                    .with_untracked(|v| v.parse().unwrap())
            ),
            device.set_noise_threshold(
                self.port,
                self.noise_threshold.with_untracked(|v| v.parse().unwrap())
            ),
            device.set_area_threshold_max(
                self.port,
                self.area_threshold_max
                    .with_untracked(|v| v.parse().unwrap())
            ),
            device.set_area_threshold_min(
                self.port,
                self.area_threshold_min
                    .with_untracked(|v| v.parse().unwrap())
            ),
            device.set_operation_mode(
                self.port,
                u8::try_from(self.operation_mode.get_untracked()).unwrap()
            ),
            device.set_max_object_cnt(
                self.port,
                self.max_object_cnt.with_untracked(|v| v.parse().unwrap())
            ),
            device.set_frame_subtraction(
                self.port,
                u8::try_from(self.frame_subtraction.get_untracked()).unwrap()
            ),
            device.set_frame_period(
                self.port,
                self.frame_period.with_untracked(|v| v.parse().unwrap())
            ),
        )?;
        tokio::try_join!(
            device.set_bank1_sync_updated(self.port, 1),
            device.set_bank0_sync_updated(self.port, 1),
        )?;
        Ok(())
    }

    fn clear(&self) {
        self.pid.update(String::clear);
        self.resolution_x.update(String::clear);
        self.resolution_y.update(String::clear);
        self.exposure_time.update(String::clear);
        self.frame_period.update(String::clear);
        self.brightness_threshold.update(String::clear);
        self.noise_threshold.update(String::clear);
        self.area_threshold_min.update(String::clear);
        self.area_threshold_max.update(String::clear);
        self.max_object_cnt.update(String::clear);

        self.operation_mode.set(0);
        self.frame_subtraction.set(0);
        self.gain.set(0);
    }

    fn load_defaults(&self) {
        self.resolution_x.update(|s| s.replace_range(.., "4095"));
        self.resolution_y.update(|s| s.replace_range(.., "4095"));
        self.frame_period.update(|s| s.replace_range(.., "49780"));
        self.brightness_threshold
            .update(|s| s.replace_range(.., "110"));
        self.noise_threshold.update(|s| s.replace_range(.., "15"));
        self.area_threshold_max
            .update(|s| s.replace_range(.., "9605"));
        self.max_object_cnt.update(|s| s.replace_range(.., "16"));

        self.operation_mode.set(0);
        self.frame_subtraction.set(0);

        match self.port {
            Port::Nf => {
                self.area_threshold_min
                    .update(|s| s.replace_range(.., "10"));
                self.exposure_time.update(|s| s.replace_range(.., "8192"));
                self.gain.set(i32::from(Gain::index_from_reg(16, 0)));
            }
            Port::Wf => {
                self.area_threshold_min.update(|s| s.replace_range(.., "5"));
                self.exposure_time.update(|s| s.replace_range(.., "11365"));
                self.gain.set(i32::from(Gain::index_from_reg(8, 3)));
            }
        }
    }
}

fn display_for_serial_port(port_info: &SerialPortInfo) -> String {
    let usb_port = match &port_info.port_type {
        serialport::SerialPortType::UsbPort(u) => u,
        _ => return port_info.port_name.clone(),
    };

    let mut out = String::new();
    if let Some(m) = &usb_port.manufacturer {
        out.push_str(m);
    }
    if let Some(p) = &usb_port.product {
        if !out.is_empty() {
            out.push_str(" - ");
        }
        out.push_str(p);
    }
    if !out.is_empty() {
        out.push_str(" ");
    }
    out.push_str(&format!("({:4x}:{:4x})", usb_port.vid, usb_port.pid));
    out = out.replace('\x00', "");
    out
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Gain {
    b_ggh: u8,
    b_global: u8,
}

impl Gain {
    const fn new(b_global: u8, b_ggh: u8) -> Self {
        Self { b_global, b_ggh }
    }
}

impl Gain {
    fn index_from_reg(b_global: u8, mut b_ggh: u8) -> i32 {
        // change b_ggh from 0,2,3 to 0,1,2
        if b_ggh > 0 {
            b_ggh -= 1;
        }
        let b_ggh = i32::from(b_ggh);
        let b_global = i32::from(b_global);
        b_ggh * 16 + b_global
    }
}

// funny

const fn n_to_bstr(n: usize) -> [u8; 6] {
    [
        '0' as u8 + (n / 10000) as u8,
        '.' as u8 as u8,
        '0' as u8 + (n / 1000 % 10) as u8,
        '0' as u8 + (n / 100 % 10) as u8,
        '0' as u8 + (n / 10 % 10) as u8,
        '0' as u8 + (n % 10) as u8,
    ]
}

pub static GAIN_TABLE: [(&str, Gain); 16 * 3 + 1] = {
    const BACKING: [[u8; 6]; 16 * 3] = {
        let mut v = [[0; 6]; 16 * 3];
        let mut i = 0;
        while i < 16 {
            v[i] = n_to_bstr((10000 + i * 10000 / 16) * 1);
            v[i + 16] = n_to_bstr((10000 + i * 10000 / 16) * 2);
            v[i + 32] = n_to_bstr((10000 + i * 10000 / 16) * 4);
            i += 1;
        }
        v
    };
    let mut omegalul = [("", Gain::new(0, 0)); 16 * 3 + 1];
    let mut i = 0;
    while i < 16 * 3 {
        omegalul[i].0 = unsafe { std::str::from_utf8_unchecked(&BACKING[i]) };
        omegalul[i].1 = Gain::new((i % 16) as u8, [0, 2, 3][i / 16]);
        i += 1;
    }
    omegalul[16 * 3] = ("8.0000", Gain::new(16, 3));
    omegalul
};

// static GAIN_TABLE: [(&'static str, Gain); 16 * 3] = [
// for j, k in zip([0, 2, 3], [1, 2, 4]):
//     for i in range(16):
//         value = (1 + i/16) * k
//         print(f'    ("{value:.4f}", Gain::new(0x{i:02x}, 0x{j:02x})),')
// ];

/// Retry an asynchronous operation up to `limit` times.
async fn retry<F, G>(mut op: F, timeout: Duration, limit: usize) -> Option<G::Output>
where
    F: FnMut() -> G,
    G: std::future::Future,
{
    for _ in 0..limit {
        match tokio::time::timeout(timeout, op()).await {
            Ok(r) => return Some(r),
            Err(_) => (),
        }
    }
    None
}
