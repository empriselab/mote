use defmt::error;
use mote_api::messages::mote_to_host::{BitList, BitResult};

pub fn update_bit_result(collection: &mut BitList, name: &'static str, result: BitResult) {
    if let Some(bit) = collection.iter_mut().find(|i| i.name == name) {
        bit.result = result;
    } else {
        error!("Failed to update Bit result for {}", name);
    }
}
