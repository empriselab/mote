mod rp_c1_driver;

use core::sync::atomic::{Ordering, compiler_fence};

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::dma::ChannelInstance;
use embassy_rp::pac::dma::vals::{DataSize, TransCountMode, TreqSel};
use embassy_rp::peripherals::DMA_CH3;
use embassy_rp::uart::{Blocking, Config, DataBits, Error as UartError, Parity, StopBits, Uart, UartRx, UartTx};
use embassy_rp::{Peri, pac};
use embassy_time::{Duration, Timer};
use mote_api::messages::mote_to_host;
use mote_api::messages::mote_to_host::{Bit, BitResult};
use static_cell::ConstStaticCell;

use super::{RplidarC1Resources, router};
use crate::helpers::update_bit_result;
use crate::tasks::lidar::rp_c1_driver::{LidarState, Point, RPLidarC1};
use crate::tasks::{CONFIGURATION_STATE, power_gate};

const MAX_POINTS_PER_SCAN_MESSAGE: usize = 100;

/// log2 of the RX ring buffer size. DMA address wrapping requires the ring to
/// be a power of two in size and naturally aligned to that size.
const RX_RING_LOG2: u8 = 12;
const RX_RING_SIZE: usize = 1 << RX_RING_LOG2;
/// How often to re-check the ring when no data is available. The C1 streams at
/// ~25 kB/s, so the 4 KiB ring holds ~160 ms of data and this is plenty fast.
const RX_POLL_INTERVAL: Duration = Duration::from_millis(1);

#[repr(C, align(4096))]
struct RxRing([u8; RX_RING_SIZE]);
const _: () = assert!(core::mem::align_of::<RxRing>() == RX_RING_SIZE);

/// UART1 with a free-running circular DMA on RX.
///
/// The DMA channel runs in RP2350 ENDLESS mode with its write address wrapping
/// inside `RxRing`, so bytes are captured continuously regardless of whether a
/// `read()` is in progress. The current DMA write address is the producer
/// index; `read_idx` is the consumer index. If the reader falls a full ring
/// behind, data is silently overwritten (the driver's per-sample check bits
/// will reject the resulting garbage).
struct DmaLidarUart<'d> {
    tx: UartTx<'d, Blocking>,
    _rx: UartRx<'d, Blocking>,
    _rx_dma: Peri<'d, DMA_CH3>,
    ring: *const u8,
    read_idx: usize,
}

impl<'d> DmaLidarUart<'d> {
    fn new(uart: Uart<'d, Blocking>, rx_dma: Peri<'d, DMA_CH3>, ring: &'d mut RxRing) -> Self {
        let (tx, rx) = uart.split();
        let ring = ring.0.as_mut_ptr();

        let ch = DMA_CH3::regs();
        ch.read_addr().write_value(pac::UART1.uartdr().as_ptr() as u32);
        ch.write_addr().write_value(ring as u32);
        ch.trans_count().write(|w| {
            w.set_mode(TransCountMode::ENDLESS);
            w.set_count(0x0fff_ffff);
        });
        compiler_fence(Ordering::SeqCst);
        ch.ctrl_trig().write(|w| {
            w.set_treq_sel(TreqSel::UART1_RX);
            w.set_data_size(DataSize::SIZE_BYTE);
            w.set_incr_read(false);
            w.set_incr_write(true);
            // Wrap the write address on a RX_RING_SIZE boundary
            w.set_ring_sel(true);
            w.set_ring_size(RX_RING_LOG2);
            // Chaining to self disables chaining
            w.set_chain_to(DMA_CH3::number());
            w.set_irq_quiet(true);
            w.set_en(true);
        });
        compiler_fence(Ordering::SeqCst);

        // Let the UART RX FIFO drive DREQ
        pac::UART1.uartdmacr().modify(|w| w.set_rxdmae(true));

        Self {
            tx,
            _rx: rx,
            _rx_dma: rx_dma,
            ring,
            read_idx: 0,
        }
    }

    fn write_idx(&self) -> usize {
        (DMA_CH3::regs().write_addr().read() as usize).wrapping_sub(self.ring as usize) & (RX_RING_SIZE - 1)
    }

    fn available(&self) -> usize {
        self.write_idx().wrapping_sub(self.read_idx) & (RX_RING_SIZE - 1)
    }
}

impl Drop for DmaLidarUart<'_> {
    fn drop(&mut self) {
        pac::UART1.uartdmacr().modify(|w| w.set_rxdmae(false));
        pac::DMA
            .chan_abort()
            .write(|w| w.set_chan_abort(1 << DMA_CH3::number()));
        while DMA_CH3::regs().ctrl_trig().read().busy() {}
    }
}

impl embedded_io_async::ErrorType for DmaLidarUart<'_> {
    type Error = UartError;
}

impl embedded_io_async::Read for DmaLidarUart<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        // Per embedded-io semantics, return as soon as at least one byte is available
        let available = loop {
            let available = self.available();
            if available > 0 {
                break available;
            }
            Timer::after(RX_POLL_INTERVAL).await;
        };
        compiler_fence(Ordering::Acquire);

        let count = available.min(buf.len());
        for (i, byte) in buf[..count].iter_mut().enumerate() {
            let idx = (self.read_idx + i) & (RX_RING_SIZE - 1);
            // SAFETY: idx is within the ring, which the DMA is concurrently writing to
            *byte = unsafe { self.ring.add(idx).read_volatile() };
        }
        self.read_idx = (self.read_idx + count) & (RX_RING_SIZE - 1);

        Ok(count)
    }
}

impl embedded_io_async::Write for DmaLidarUart<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        // LiDAR requests are only a couple of bytes, so they always fit in the
        // 32-byte TX FIFO without actually blocking.
        self.tx.blocking_write(buf)?;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        while self.tx.busy() {
            Timer::after_micros(10).await;
        }
        Ok(())
    }
}

impl From<Point> for mote_to_host::Point {
    fn from(value: Point) -> Self {
        mote_to_host::Point {
            quality: value.quality,
            // Feels expensive, but the RP2354 has a FPU so probably fine
            angle_rad: (value.angle as f32 / 64.0).to_radians(),
            distance_mm: value.distance as f32 / 4.0,
        }
    }
}

#[embassy_executor::task]
async fn lidar_state_machine_task(r: RplidarC1Resources) {
    // Init Bit
    {
        let mut configuration_state = CONFIGURATION_STATE.lock().await;
        let init = Bit {
            name: "Init".into(),
            result: BitResult::Waiting,
        };
        let check_health = Bit {
            name: "Check Health".into(),
            result: BitResult::Waiting,
        };
        for test in [init, check_health] {
            configuration_state.built_in_test.lidar.push(test);
        }
    }

    info!("Gating on 1.5A capable before starting LiDAR");
    power_gate::gate_1_5_amp().await;
    info!("Power supply is 1.5A capable");

    let mut config = Config::default();
    config.baudrate = 460800;
    config.stop_bits = StopBits::STOP1;
    config.data_bits = DataBits::DataBits8;
    config.parity = Parity::ParityNone;

    let uart = Uart::new_blocking(r.uart, r.tx, r.rx, config);

    static RX_RING: ConstStaticCell<RxRing> = ConstStaticCell::new(RxRing([0; RX_RING_SIZE]));
    // Must not be constructed at runtime: a 4 KiB, 4 KiB-aligned temporary in
    // this task's frame overflows core 1's stack.
    let rx_ring = RX_RING.take();

    let mut state = LidarState::Reset;

    let mut point_buf: [rp_c1_driver::Point; MAX_POINTS_PER_SCAN_MESSAGE] = [rp_c1_driver::Point::default(); _];
    let mut valid_points = 0;

    let mut driver = RPLidarC1::new(DmaLidarUart::new(uart, r.rx_dma, rx_ring));

    // Update init state
    {
        let mut configuration_state = CONFIGURATION_STATE.lock().await;
        update_bit_result(&mut configuration_state.built_in_test.lidar, "Init", BitResult::Pass);
    }

    loop {
        state = match state {
            LidarState::Idle => LidarState::Idle,
            LidarState::Start => LidarState::Reset,
            LidarState::Reset => driver.reset().await,
            LidarState::CheckHealth => {
                let next_state = driver.check_health().await;
                {
                    let mut configuration_state = CONFIGURATION_STATE.lock().await;
                    update_bit_result(
                        &mut configuration_state.built_in_test.lidar,
                        "Check Health",
                        if next_state == LidarState::Reset {
                            BitResult::Fail
                        } else {
                            BitResult::Pass
                        },
                    );
                }
                next_state
            }
            LidarState::ScanRequest => driver.scan_request().await,
            // This could be updated to use zerocopy for a nice performance boost
            LidarState::ReceiveSample => {
                match driver.receive_samples(&mut point_buf).await {
                    Ok(count) => {
                        if count < (MAX_POINTS_PER_SCAN_MESSAGE >> 1) {
                            // More than 50% of points were read incorrectly
                            LidarState::CheckHealth
                        } else {
                            valid_points = count;
                            LidarState::ProcessSample
                        }
                    }
                    Err(_) => {
                        // Something is wrong, check health and try again
                        LidarState::CheckHealth
                    }
                }
            }
            LidarState::ProcessSample => {
                // We don't care if these packets get lost, so don't block if the channel is
                // full

                // let _ = DATA_OFFLOAD_CHANNEL.try_send(mote_to_host::Message::Scan(
                //    point_buf[..valid_points].iter().map(|&point| point.into()).collect(),
                // ));

                let timestamp = router::get_timestamp();
                let point_cloud: router::PointCloud = point_buf[..valid_points]
                    .iter()
                    .filter_map(|&point| {
                        if point.quality >= 20 {
                            let scan_point: mote_to_host::Point = point.into();
                            Some(router::ScanPoint {
                                quality: scan_point.quality,
                                angle: units::PlaneAngle::new(scan_point.angle_rad),
                                distance: units::Length::new(scan_point.distance_mm / 1000.0_f32),
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                router::TO_GNC_CHAN
                    .send(router::Message::LidarScan(router::Scan { timestamp, point_cloud }))
                    .await;

                LidarState::ReceiveSample
            }
            LidarState::Stop => LidarState::Reset,
        }
    }
}

pub async fn init(spawner: Spawner, r: RplidarC1Resources) {
    // Start task
    spawner.spawn(lidar_state_machine_task(r).unwrap());
}
