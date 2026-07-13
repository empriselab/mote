use defmt::{error, info, warn};
use embassy_futures::select::{Either, select};
use embassy_net::Stack;
use embassy_net::udp::{PacketMetadata, UdpMetadata, UdpSocket};
use mote_api::HostLink;
use mote_api::messages::{host_to_mote, mote_to_host};

use crate::tasks::wifi::{DATA_OFFLOAD_CHANNEL, MOTOR_COMMAND_CHANNEL};

pub const UDP_SERVER_PORT: u16 = 7475;

async fn handle_command(rx_message: host_to_mote::Message, link: &mut HostLink) {
    match rx_message {
        host_to_mote::Message::Ping => {
            info!("Parsed ping request, responding.");
            let _ = link.send(mote_to_host::Message::Pong);
        }
        host_to_mote::Message::Pong => {
            info!("Received ping response from host.");
        }
        host_to_mote::Message::SetDriveBaseVelocity(cmd) => {
            let _ = MOTOR_COMMAND_CHANNEL.try_send(cmd);
        }
        _ => {
            error!("Received unhandled message type");
        }
    }
}

#[embassy_executor::task]
pub async fn udp_server_task(stack: Stack<'static>) -> ! {
    // Wait for IPV4 to come up
    stack.wait_link_up().await;
    stack.wait_config_up().await;

    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 4096];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 4096];
    let mut socket = UdpSocket::new(stack, &mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer);

    if let Err(e) = socket.bind(UDP_SERVER_PORT) {
        warn!("bind error: {:?}", e);
    }

    let mut link = HostLink::new();
    let mut message_buffer = [0; 4096];
    let mut client: Option<UdpMetadata> = None;

    loop {
        match select(socket.recv_from(&mut message_buffer), DATA_OFFLOAD_CHANNEL.receive()).await {
            Either::First(Ok((bytes_read, ep))) => {
                let new_client = match client {
                    None => {
                        info!("Client connected: {}", ep);
                        true
                    }
                    Some(ref current) if *current != ep => {
                        info!("Client changed: {} -> {}", current, ep);
                        true
                    }
                    _ => false,
                };
                if new_client {
                    client = Some(ep);
                }

                link.handle_receive(&message_buffer[..bytes_read]);
                loop {
                    match link.poll_receive() {
                        Ok(Some(message)) => handle_command(message, &mut link).await,
                        Ok(None) => break,
                        Err(mote_api::Error::VersionMismatch {
                            local,
                            remote,
                            local_role,
                            remote_role,
                            behind,
                        }) => {
                            warn!(
                                "Dropped message: mote-api version mismatch ({} v{}.{}.{}, {} v{}.{}.{}) — update {}",
                                local_role.as_str(),
                                local.major,
                                local.minor,
                                local.patch,
                                remote_role.as_str(),
                                remote.major,
                                remote.minor,
                                remote.patch,
                                behind.as_str(),
                            );
                        }
                        Err(_) => warn!("Dropped undecodable message"),
                    }
                }

                while let Some(payload) = link.poll_transmit() {
                    if let Err(err) = socket.send_to(&payload, ep).await {
                        error!("UDP send error: {}", err);
                    }
                }
            }
            Either::First(Err(err)) => {
                error!("UDP recv error: {}", err);
            }
            Either::Second(message) => {
                if let Some(ep) = client {
                    link.send(message).unwrap();

                    while let Some(payload) = link.poll_transmit() {
                        if let Err(err) = socket.send_to(&payload, ep).await {
                            if matches!(err, embassy_net::udp::SendError::NoRoute) {
                                info!("Client disconnected: {}", ep);
                                client = None;
                            } else {
                                error!("UDP send error: {}", err);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
}
