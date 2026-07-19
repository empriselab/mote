use embassy_executor::Spawner;
use embassy_rp::i2c::{Config, I2c};
use embassy_rp::peripherals::I2C1;
use lsm6ds33::{
    AccelerometerOutput, AccelerometerScale, Error as ImuError, GyroscopeFullScale, GyroscopeOutput, Lsm6ds33Async,
};
use mote_api::messages::mote_to_host;
use mote_api::messages::mote_to_host::{Bit, BitResult, ImuAxisTriple, ImuMeasurement};

use super::{ImuResources, Irqs};
use crate::helpers::update_bit_result;
use crate::tasks::CONFIGURATION_STATE;
use crate::wifi::DATA_OFFLOAD_CHANNEL;

// NUMBER OF MISSED IMU READS IN A ROW BEFORE WE FLAG A Bit FAILURE
const MISSED_READ_THRESHOLD: u8 = 10;
const INVALID_TEMPERATURE: f32 = 25.0; // value returned by get_sensor_data on read failure 
// (MUST be 25.0 since imu.read_all() may not return an error but just return 0
// for all values, in this case the temp is read as 25)

fn default_measurement() -> ImuMeasurement {
    ImuMeasurement {
        accel: ImuAxisTriple { x: 0.0, y: 0.0, z: 0.0 },
        gyro: ImuAxisTriple { x: 0.0, y: 0.0, z: 0.0 },
    }
}

// Returns temperature and (accel, gyro) IMU measurement, or None if neither
// sensor has produced a fresh sample since the last read (expected occasionally
// since the poll loop period doesn't evenly divide the sensor's output data
// rate).
pub async fn get_sensor_data(
    imu: &mut Lsm6ds33Async<I2c<'static, I2C1, embassy_rp::i2c::Async>>,
) -> Option<(f32, ImuMeasurement)> {
    match (imu.accel_data_available().await, imu.gyro_data_available().await) {
        (Ok(false), Ok(_)) | (Ok(_), Ok(false)) => None,
        (Ok(true), Ok(true)) => Some(match imu.read_all().await {
            Ok((temperature, gyro_tuple, accel_tuple)) => {
                // Map the accelerometer tuple (f32, f32, f32) to ImuAxisTriple
                let accel = ImuAxisTriple {
                    x: accel_tuple.0,
                    y: accel_tuple.1,
                    z: accel_tuple.2,
                };

                // Map the gyroscope tuple (f32, f32, f32) to ImuAxisTriple
                let gyro = ImuAxisTriple {
                    x: gyro_tuple.0,
                    y: gyro_tuple.1,
                    z: gyro_tuple.2,
                };

                // Return the temperature and the combined measurement
                (temperature, ImuMeasurement { accel, gyro })
            }
            // Default error case
            Err(_) => (INVALID_TEMPERATURE, default_measurement()), // invalid temperature to indicate error
        }),
        // Communication error while checking sensor status
        _ => Some((INVALID_TEMPERATURE, default_measurement())),
    }
}

#[embassy_executor::task]
async fn imu_task(r: ImuResources) {
    let i2c = I2c::new_async(r.i2c, r.scl, r.sda, Irqs, Config::default());
    let mut imu = reset_imu(i2c).await;
    let mut missed_read_count: u8 = 0;

    // Sensor Reading loop
    loop {
        if let Some((temp, measurement)) = get_sensor_data(&mut imu).await {
            let _ = DATA_OFFLOAD_CHANNEL.try_send(mote_to_host::Message::ImuMeasurement(measurement));

            // get sensor data errored, update Bit and log, and missed read count
            if temp == INVALID_TEMPERATURE {
                missed_read_count += 1;
                defmt::error!(
                    "Failed to read IMU sensor data. Missed reads in a row: {}, Waiting 5 seconds before attempting recovery",
                    missed_read_count
                );
                if missed_read_count >= MISSED_READ_THRESHOLD {
                    {
                        let mut configuration_state = CONFIGURATION_STATE.lock().await;
                        update_bit_result(
                            &mut configuration_state.built_in_test.imu,
                            "Reading Values",
                            BitResult::Fail,
                        );
                    }
                    // waiting here for a few seconds to allow the Bit state to be observed as
                    // failed before attempting recovery
                    embassy_time::Timer::after_secs(5).await;

                    // reclaim i2c resources
                    let i2c = imu.release();
                    imu = reset_imu(i2c).await; // attempt to reset the IMU after hitting the missed read threshold
                }
            } else {
                missed_read_count = 0; // reset missed read count on successful read
            }
        }
        // if get_sensor_data returned None, neither sensor had a fresh sample yet;
        // skip this cycle without counting it as a missed read

        embassy_time::Timer::after_millis(20).await;
    }
}

async fn reset_imu(
    mut i2c: I2c<'static, I2C1, embassy_rp::i2c::Async>,
) -> Lsm6ds33Async<I2c<'static, I2C1, embassy_rp::i2c::Async>> {
    loop {
        defmt::info!("Resetting IMU");

        i2c = match Lsm6ds33Async::new(i2c, 0x6A).await {
            Ok(mut driver) => {
                let config_res: Result<(), ImuError<embassy_rp::i2c::Error>> = async {
                    // ODR matches the ~50Hz task poll rate (Timer::after_millis(20) below)
                    driver.set_accelerometer_output(AccelerometerOutput::Rate52).await?;
                    driver.set_accelerometer_scale(AccelerometerScale::G04).await?;
                    driver.set_gyroscope_output(GyroscopeOutput::Rate52).await?;
                    driver.set_gyroscope_scale(GyroscopeFullScale::Dps500).await?;
                    Ok(())
                }
                .await;

                match config_res {
                    Ok(_) => {
                        defmt::info!("IMU Initialized and Configured");
                        {
                            let mut state = CONFIGURATION_STATE.lock().await;
                            update_bit_result(&mut state.built_in_test.imu, "Init", BitResult::Pass);
                            update_bit_result(&mut state.built_in_test.imu, "Reading Values", BitResult::Pass);
                        }
                        return driver;
                    }
                    Err(e) => {
                        match e {
                            ImuError::CommunicationError(_) => {
                                defmt::error!("IMU Error: I2C Communication failed during config")
                            }
                            _ => defmt::error!("IMU Error: Unknown config error"),
                        }
                        driver.release()
                    }
                }
            }
            Err((returned_bus, e)) => {
                match e {
                    ImuError::CommunicationError(_) => defmt::error!("IMU Error: I2C Communication failed during init"),
                    ImuError::ChipDetectFailed => defmt::error!("IMU Error: Chip not detected"),
                    _ => defmt::error!("IMU Error: Unknown init error"),
                }
                returned_bus
            }
        };

        {
            let mut state = CONFIGURATION_STATE.lock().await;
            update_bit_result(&mut state.built_in_test.imu, "Init", BitResult::Fail);
        }

        defmt::warn!("IMU recovery: Retrying in 5 seconds...");
        embassy_time::Timer::after_secs(5).await;
    }
}

pub async fn init(spawner: Spawner, r: ImuResources) {
    // setup bit state for config page
    {
        let mut configuration_state = CONFIGURATION_STATE.lock().await;
        let init_bit = Bit {
            name: "Init".into(),
            result: BitResult::Waiting,
        };
        let health_bit = Bit {
            name: "Reading Values".into(),
            result: BitResult::Waiting,
        };
        configuration_state.built_in_test.imu.push(init_bit);
        configuration_state.built_in_test.imu.push(health_bit);
    }

    spawner.spawn(imu_task(r).unwrap());
}
