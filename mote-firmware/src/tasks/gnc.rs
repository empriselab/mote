use embassy_time::{Duration, with_timeout};

use super::router;

pub async fn init(spawner: embassy_executor::Spawner) {
    spawner.spawn(gnc_task().expect("gnc_task already spawned"));
}

#[embassy_executor::task]
async fn gnc_task() {
    let mut context = Context::default();
    context.execute().await;
}

const SAMPLE_REPORT_DECIMATION_VALUE: u8 = 25;

#[derive(Default)]
struct Context {
    guidance_context: mote_api::work::guidance::Context,
    control_context: mote_api::work::control::Context,
    navigation_context: mote_api::work::navigation::Context,
    route: Option<router::Route>,
    control_mode: router::ControlMode,
    position: Option<router::Position>,
    reference_frame: Option<router::ReferenceFrame>,
    telemetry_selection: router::TelemetrySelection,
    imu_telemetry_decimator: router::ImuTelemetryDecimator,
    imu_sample_counter: u8,
    lidar_scan_counter: u8,
    status: router::Status,
}

impl Context {
    async fn handle_lidar_scan(&mut self, scan: router::Scan) -> Option<router::Position> {
        if self.lidar_scan_counter == SAMPLE_REPORT_DECIMATION_VALUE {
            router::update_counter(
                &mut self.status.counters.lidar_scan,
                SAMPLE_REPORT_DECIMATION_VALUE as u32,
            );
            self.lidar_scan_counter = 0;
        }
        if self.telemetry_selection.is_send_lidar() {
            let _ = router::TO_WIFI_CHAN.try_send(router::Message::LidarScan(scan.clone()));
        }
        if let Some(reference_frame) = self.reference_frame.as_ref() {
            let position = self.navigation_context.update(
                router::get_timestamp(),
                self.position.as_ref(),
                Some(scan),
                None,
                reference_frame,
                &mut self.status,
            );
            if position.is_some() {
                router::update_counter(
                    &mut self.status.counters.navigation_lidar_position_counter,
                    SAMPLE_REPORT_DECIMATION_VALUE as u16,
                );
            }
            position
        } else {
            None
        }
    }

    async fn handle_imu(&mut self, imu: router::Imu) -> Option<router::Position> {
        self.imu_sample_counter += 1;
        if self.imu_sample_counter == SAMPLE_REPORT_DECIMATION_VALUE {
            router::update_counter(
                &mut self.status.counters.imu_sample,
                SAMPLE_REPORT_DECIMATION_VALUE as u32,
            );
            self.imu_sample_counter = 0;
        }
        if self.imu_telemetry_decimator.should_send(self.telemetry_selection) {
            let _ = router::TO_WIFI_CHAN.try_send(router::Message::Imu(imu.clone()));
        }
        if let Some(reference_frame) = self.reference_frame.as_ref() {
            self.navigation_context.update(
                router::get_timestamp(),
                self.position.as_ref(),
                None,
                Some(imu),
                reference_frame,
                &mut self.status,
            )
        } else {
            None
        }
    }

    async fn send_position(&self, position: router::Position) {
        let _ = router::TO_WIFI_CHAN.try_send(router::Message::Position(position));
    }

    /// Forwards `motor_drive` to the motors only if `source` is the active
    /// control mode.
    async fn send_motor_command(&self, source: router::ControlMode, motor_drive: router::MotorDrive) {
        if self.control_mode == source {
            router::MOTOR_COMMAND_CHANNEL.send(motor_drive.into()).await;
        }
    }

    async fn stop_motors(&self) {
        let stop = router::MotorDrive {
            left: units::AngularVelocity::new(0.0),
            right: units::AngularVelocity::new(0.0),
        };
        router::MOTOR_COMMAND_CHANNEL.send(stop.into()).await;
    }

    async fn set_control_mode(&mut self, control_mode: router::ControlMode) {
        defmt::info!("Received control mode command: {}", control_mode);
        self.status.set_log(match control_mode {
            router::ControlMode::None => "Received control mode: None",
            router::ControlMode::Remote => "Received control mode: Remote",
            router::ControlMode::Auto => "Received control mode: Auto",
        });
        // Stop on every mode change so commands from the previous source don't keep
        // driving the motors, and always stop when `None` is received.
        let mode_changed = self.control_mode != control_mode;
        self.control_mode = control_mode;
        if mode_changed || control_mode == router::ControlMode::None {
            self.stop_motors().await;
        }
    }

    async fn send_guidance(&self, guidance: Option<&router::Guidance>) {
        if let Some(guidance) = guidance {
            let _ = router::TO_WIFI_CHAN.try_send(router::Message::Guidance(guidance.clone()));
        }
    }

    async fn send_control(&self, control: router::Control) {
        let _ = router::TO_WIFI_CHAN.try_send(router::Message::Control(control));
    }

    async fn handle_update(&mut self) {
        let timestamp = router::get_timestamp();
        if let (Some(route), Some(position)) = (self.route.as_ref(), self.position.as_ref()) {
            let guidance = self
                .guidance_context
                .update(timestamp, position, route, &mut self.status);
            self.send_guidance(guidance.as_ref()).await;
            // Without a reference frame control cannot convert guidance into wheel
            // speeds; the default frame makes it command a stop.
            let reference_frame = self.reference_frame.clone().unwrap_or_default();
            if let Some(control) = self
                .control_context
                .update(timestamp, guidance, &reference_frame, &mut self.status)
            {
                self.send_control(control.clone()).await;
                self.send_motor_command(router::ControlMode::Auto, control.motor_drive)
                    .await;
            }
        }
    }

    async fn handle_message(&mut self, message: router::Message) {
        let position = match message {
            router::Message::None => None,
            router::Message::ControlMode(control_mode) => {
                self.set_control_mode(control_mode).await;
                None
            }
            router::Message::TelemetrySelection(telemetry_selection) => {
                self.telemetry_selection = telemetry_selection;
                None
            }
            router::Message::ReferenceFrame(reference_frame) => {
                self.status.set_log("Received reference frame");
                self.status.reference_frame_hash = reference_frame.hash();
                self.reference_frame = Some(reference_frame);
                None
            }
            router::Message::MotorDrive(motor_drive) => {
                self.send_motor_command(router::ControlMode::Remote, motor_drive).await;
                None
            }
            router::Message::Imu(imu) => self.handle_imu(imu).await,
            router::Message::LidarScan(scan) => self.handle_lidar_scan(scan).await,
            router::Message::Position(position) => {
                self.position = Some(position);
                None
            }
            router::Message::Route(route) => {
                self.status.set_log("Received the route");
                self.status.route_hash = router::hash_route(&route);
                self.route = Some(route);
                None
            }
            router::Message::ResetRoute => {
                self.guidance_context.reset_progress();
                self.status.set_log("Reset the route");
                None
            }
            _ => None,
        };
        if let Some(position) = position {
            self.send_position(position.clone()).await;
            self.position = Some(position);
        }
        self.handle_update().await;
    }

    async fn execute(&mut self) {
        let mut next_debug_hash = 0;
        loop {
            let debug_hash = self.status.hash();
            let timeout_result = with_timeout(Duration::from_millis(50), router::TO_GNC_CHAN.receive()).await;
            match timeout_result {
                Ok(message) => self.handle_message(message).await,
                Err(_) => self.handle_update().await,
            }
            if next_debug_hash != debug_hash {
                next_debug_hash = debug_hash;
                let _ = router::TO_WIFI_CHAN.try_send(router::Message::Debug(self.status.clone()));
                self.status.sequence += 1;
                self.status.log = heapless::String::new();
            }
        }
    }
}
