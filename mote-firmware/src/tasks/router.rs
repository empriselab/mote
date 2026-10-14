use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
pub use mote_api::messages::router::*;

pub use super::wifi::MOTOR_COMMAND_CHANNEL;

pub static TO_WIFI_CHAN: Channel<CriticalSectionRawMutex, Message, 4> = Channel::new();
pub static TO_GNC_CHAN: Channel<CriticalSectionRawMutex, Message, 4> = Channel::new();

pub fn get_timestamp() -> units::Time {
    units::Time::new(embassy_time::Instant::now().as_millis() as f32 / 1000.0)
}
