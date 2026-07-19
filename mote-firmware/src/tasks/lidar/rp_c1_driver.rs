// This really should be an independent crate.
// https://github.com/cnwzhjs/rplidar.rs/tree/master exists but requires std and isn't embedded_hal
// compatible. But it does support the S3 / S2 LiDARs.
// This implementation only covers the C1, but could extended with a little
// work.

use defmt::{Format, error, warn};
use embassy_time::{Duration, TimeoutError, Timer, with_timeout};
use embedded_io_async::{ErrorType, ReadExactError};

const START_FLAG: u8 = 0xA5;

/// Fixed 7-byte header of the LiDAR's response to a GET_HEALTH request: start
/// flag, sync byte, a little-endian 30-bit length/mode field (constant value
/// 3), and the GET_HEALTH data type byte.
const HEALTH_RESPONSE_HEADER: [u8; 7] = [0xA5, 0x5A, 0x03, 0x00, 0x00, 0x00, 0x06];

/// Fixed 7-byte response to a START_SCAN request. Unlike
/// `HEALTH_RESPONSE_HEADER`, this is the LiDAR's entire response, not just a
/// header prefix.
const START_SCAN_RESPONSE: [u8; 7] = [0xA5, 0x5A, 0x05, 0x00, 0x00, 0x40, 0x81];

/// Bounds how many leading bytes `sync_to_header` will discard while
/// re-synchronizing before giving up. 64 matches the capacity of the UART's
/// software RX buffer (`RX_BUF` in `src/tasks/lidar.rs`), the worst case amount
/// of stray data that could still be buffered despite `clear_read()`.
const MAX_RESYNC_SKIP: usize = 64;

#[non_exhaustive]
struct Requests;

#[allow(dead_code)]
impl Requests {
    pub const STOP: [u8; 2] = [START_FLAG, 0x25];
    pub const RESET: [u8; 2] = [START_FLAG, 0x40];
    pub const SCAN: [u8; 2] = [START_FLAG, 0x20];
    pub const EXPRESS_SCAN: [u8; 2] = [START_FLAG, 0x82];
    pub const GET_INFO: [u8; 2] = [START_FLAG, 0x50];
    pub const GET_HEALTH: [u8; 2] = [START_FLAG, 0x52];
    pub const GET_SAMPLE_RATE: [u8; 2] = [START_FLAG, 0x59];
    pub const GET_LIDAR_CONF: [u8; 2] = [START_FLAG, 0x84];
}

#[derive(PartialEq, Eq)]
#[allow(dead_code)]
pub enum LidarState {
    Idle,
    Start,
    Reset,
    CheckHealth,
    ScanRequest,
    ReceiveSample,
    ProcessSample,
    Stop,
}

#[derive(Debug, defmt::Format, Clone, Default, Copy)]
pub struct Point {
    pub quality: u8,
    // Actual heading = angle / 64.0 degrees
    pub angle: u16,
    // Actual distance = distance / 4.0 mm
    pub distance: u16,
}

#[allow(dead_code)]
pub enum ReadSamplesError<T> {
    Timeout,
    CheckBitIncorrect,
    StartFlagIncorrect,
    IoError(T),
}

/// Error from `RPLidarC1::sync_to_header`.
enum SyncError<E> {
    /// An IO error occurred while reading from the connection.
    Io(ReadExactError<E>),
    /// `MAX_RESYNC_SKIP` bytes were discarded without finding the expected
    /// header/response. Carries the last 7-byte window examined for
    /// diagnostics.
    NotFound([u8; 7]),
}

pub struct RPLidarC1<T>
where
    T: embedded_io_async::Write + embedded_io_async::Read,
{
    connection: T,
}

impl<T> RPLidarC1<T>
where
    T: embedded_io_async::Write + embedded_io_async::Read,
    <T as ErrorType>::Error: Format,
{
    pub fn new(connection: T) -> Self {
        Self { connection }
    }

    async fn clear_read(&mut self) {
        let mut resp = [0; 256];
        while let Ok(Ok(256)) = with_timeout(Duration::from_millis(200), self.connection.read(&mut resp)).await {
            // We read a full buffer, there might be more to read. Try again
        }
    }

    pub async fn reset(&mut self) -> LidarState {
        match self.connection.write_all(&Requests::RESET).await {
            Ok(_) => {
                // Delay to give the LiDAR time to reboot
                Timer::after_millis(1000).await;

                // Clear the UART buffer
                self.clear_read().await;

                LidarState::CheckHealth
            }
            Err(err) => {
                // Otherwise we have an error, attempt to reset again after a short delay
                error!("Failed to send RESET command to LiDAR ({}), retrying...", err);
                Timer::after_millis(1000).await;
                LidarState::Reset
            }
        }
    }

    /// Reads bytes one at a time, sliding a 7-byte window across the incoming
    /// stream, until the window matches `header`, discarding any leading
    /// junk bytes. Returns the number of leading bytes discarded before the
    /// header was found (0 if it started immediately), or
    /// `SyncError::NotFound` once `MAX_RESYNC_SKIP` bytes have been discarded
    /// without a match.
    async fn sync_to_header(&mut self, header: &[u8; 7]) -> Result<usize, SyncError<<T as ErrorType>::Error>> {
        let mut window = [0u8; 7];
        self.connection.read_exact(&mut window).await.map_err(SyncError::Io)?;

        let mut skipped = 0;
        while window != *header {
            if skipped >= MAX_RESYNC_SKIP {
                return Err(SyncError::NotFound(window));
            }
            window.copy_within(1.., 0);
            self.connection
                .read_exact(&mut window[6..])
                .await
                .map_err(SyncError::Io)?;
            skipped += 1;
        }

        Ok(skipped)
    }

    pub async fn check_health(&mut self) -> LidarState {
        // Clear the UART buffer
        self.clear_read().await;

        match self.connection.write_all(&Requests::GET_HEALTH).await {
            Ok(()) => {
                match with_timeout(Duration::from_millis(500), self.sync_to_header(&HEALTH_RESPONSE_HEADER)).await {
                    Ok(Ok(skipped)) => {
                        if skipped > 0 {
                            warn!(
                                "Discarded {} leading junk byte(s) before finding LiDAR GET_HEALTH response header",
                                skipped
                            );
                        }

                        let mut payload = [0u8; 3];
                        match with_timeout(Duration::from_millis(200), self.connection.read_exact(&mut payload)).await {
                            Ok(Ok(())) => match payload[0] {
                                0x00 => {
                                    return LidarState::ScanRequest;
                                }
                                status => {
                                    let mut error: [u8; 2] = [0; 2];
                                    error.copy_from_slice(&payload[0..2]);
                                    error!(
                                        "LiDAR GET_HEALTH returned status code {} and error code {}",
                                        status,
                                        u16::from_le_bytes(error)
                                    );
                                }
                            },
                            Ok(Err(err)) => {
                                error!("Failed to read GET_HEALTH response payload from LiDAR ({})", err);
                            }
                            Err(TimeoutError) => {
                                error!("Timed out reading GET_HEALTH response payload from LiDAR");
                            }
                        }
                    }
                    Ok(Err(SyncError::NotFound(window))) => {
                        error!(
                            "Could not find LiDAR GET_HEALTH response header within {} bytes (last window: {:#x}), resetting...",
                            MAX_RESYNC_SKIP, window
                        );
                    }
                    Ok(Err(SyncError::Io(err))) => {
                        error!("Failed to read GET_HEALTH response from LiDAR ({})", err);
                    }
                    Err(TimeoutError) => {
                        error!("Timed out waiting for LiDAR GET_HEALTH response header");
                    }
                }
            }
            Err(err) => {
                error!("Failed to send GET_HEALTH command to LiDAR ({}), resetting...", err);
            }
        }

        LidarState::Reset
    }

    pub async fn scan_request(&mut self) -> LidarState {
        match self.connection.write_all(&Requests::SCAN).await {
            Ok(()) => match with_timeout(Duration::from_millis(500), self.sync_to_header(&START_SCAN_RESPONSE)).await {
                Ok(Ok(skipped)) => {
                    if skipped > 0 {
                        warn!(
                            "Discarded {} leading junk byte(s) before finding LiDAR START_SCAN response",
                            skipped
                        );
                    }
                    return LidarState::ReceiveSample;
                }
                Ok(Err(SyncError::NotFound(window))) => {
                    warn!(
                        "Could not find LiDAR START_SCAN response within {} bytes (last window: {:#x}), checking health...",
                        MAX_RESYNC_SKIP, window
                    );
                }
                Ok(Err(SyncError::Io(err))) => {
                    warn!(
                        "Failed to read START_SCAN response from LiDAR ({}), checking health...",
                        err
                    );
                }
                Err(TimeoutError) => {
                    warn!("Timed out waiting for LiDAR START_SCAN response, checking health...");
                }
            },
            Err(err) => {
                warn!(
                    "Failed to send START_SCAN command to LiDAR ({}), checking health...",
                    err
                );
            }
        }
        LidarState::CheckHealth
    }

    pub async fn receive_samples<const N: usize>(
        &mut self,
        point_buf: &mut [Point; N],
    ) -> Result<usize, ReadSamplesError<ReadExactError<<T as ErrorType>::Error>>>
    where
        [(); 5 * N]:,
    {
        let mut idx = 0;

        let mut buffer = [0; 5 * N];
        match with_timeout(Duration::from_millis(5000), self.connection.read_exact(&mut buffer)).await {
            Ok(Ok(())) => {
                for i in 0..N {
                    let resp = &buffer[(i * 5)..(i * 5) + 5];
                    if resp[0] & 0b01 == resp[0] & 0b10 {
                        warn!("Start flag data check failed for LiDAR data message.");
                        continue;
                    } else if resp[1] & 0b1 != 1 {
                        error!("Check bit data check failed for LiDAR data message.");
                        continue;
                    } else {
                        let angle = ((resp[2] as u16) << 7) | ((resp[1] as u16 & 0xFE) >> 1);

                        let mut distance_bytes: [u8; 2] = [0; 2];
                        distance_bytes.copy_from_slice(&resp[3..5]);
                        let distance = u16::from_le_bytes(distance_bytes);

                        point_buf[idx] = Point {
                            quality: (resp[0] & !0b11) >> 2,
                            angle,
                            distance,
                        };
                        idx += 1;
                    }
                }

                Ok(idx)
            }
            Ok(Err(err)) => {
                error!("Failed to read point from LiDAR ({}), resetting...", err);
                Err(ReadSamplesError::IoError(err))
            }
            Err(TimeoutError) => Ok(0),
        }
    }
}
