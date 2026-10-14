use defmt::{error, info, warn};
use embassy_futures::select::{Either3, select3};
use embassy_net::Stack;
use embassy_net::udp::{PacketMetadata, UdpMetadata, UdpSocket};
use mote_api::HostLink;
use mote_api::messages::{host_to_mote, mote_to_host};

use crate::tasks::router;
use crate::tasks::wifi::{DATA_OFFLOAD_CHANNEL, MOTOR_COMMAND_CHANNEL};

pub const UDP_SERVER_PORT: u16 = 7475;

#[embassy_executor::task]
pub(super) async fn udp_server_task(stack: Stack<'static>) -> ! {
    stack.wait_link_up().await;
    stack.wait_config_up().await;
    let mut context_zero = UdpServerContextZero::new();
    let mut context = UdpServerContext::new(stack, &mut context_zero);
    context.execute().await
}

pub(super) struct UdpServerContextZero {
    rx_meta: [PacketMetadata; 16],
    rx_buffer: [u8; 4096],
    tx_meta: [PacketMetadata; 16],
    tx_buffer: [u8; 4096],
}

impl UdpServerContextZero {
    pub(super) const fn new() -> Self {
        Self {
            rx_meta: [PacketMetadata::EMPTY; 16],
            rx_buffer: [0; 4096],
            tx_meta: [PacketMetadata::EMPTY; 16],
            tx_buffer: [0; 4096],
        }
    }
}

pub(super) struct UdpServerContext<'a> {
    socket: UdpSocket<'a>,
    link: HostLink,
    message_buffer: [u8; 4096],
    client: Option<UdpMetadata>,
}

impl<'a> UdpServerContext<'a> {
    fn new(stack: Stack<'static>, context_zero: &'a mut UdpServerContextZero) -> Self {
        let socket = UdpSocket::new(
            stack,
            &mut context_zero.rx_meta,
            &mut context_zero.rx_buffer,
            &mut context_zero.tx_meta,
            &mut context_zero.tx_buffer,
        );

        Self {
            socket,
            link: HostLink::new(),
            message_buffer: [0; 4096],
            client: None,
        }
    }

    async fn execute(&mut self) -> ! {
        if let Err(e) = self.socket.bind(UDP_SERVER_PORT) {
            warn!("bind error: {:?}", e);
        }

        loop {
            match select3(
                self.socket.recv_from(&mut self.message_buffer),
                DATA_OFFLOAD_CHANNEL.receive(),
                router::TO_WIFI_CHAN.receive(),
            )
            .await
            {
                Either3::First(Ok((bytes_read, ep))) => self.handle_received_datagram(bytes_read, ep).await,
                Either3::First(Err(err)) => {
                    error!("UDP recv error: {}", err);
                }
                Either3::Second(message) => self.handle_data_offload(message).await,
                Either3::Third(telemetry_message) => {
                    self.send_to_client(mote_to_host::Message::PubSub(telemetry_message))
                        .await
                }
            }
        }
    }

    async fn handle_received_datagram(&mut self, bytes_read: usize, endpoint: UdpMetadata) {
        let new_client = match self.client {
            None => {
                info!("Client connected: {}", endpoint);
                true
            }
            Some(ref current) if *current != endpoint => {
                info!("Client changed: {} -> {}", current, endpoint);
                true
            }
            _ => false,
        };
        if new_client {
            self.client = Some(endpoint);
        }

        self.link.handle_receive(&self.message_buffer[..bytes_read]);
        loop {
            match self.link.poll_receive() {
                Ok(Some(message)) => {
                    self.handle_command(message).await;
                }
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

        while let Some(payload) = self.link.poll_transmit() {
            if let Err(err) = self.socket.send_to(&payload, endpoint).await {
                error!("UDP send error: {}", err);
            }
        }
    }

    async fn handle_data_offload(&mut self, message: mote_to_host::Message) {
        self.send_to_client(message).await;
    }

    async fn send_to_client(&mut self, message: mote_to_host::Message) {
        if let Some(endpoint) = self.client {
            self.link.send(message).unwrap();

            while let Some(payload) = self.link.poll_transmit() {
                if let Err(err) = self.socket.send_to(&payload, endpoint).await {
                    if matches!(err, embassy_net::udp::SendError::NoRoute) {
                        info!("Client disconnected: {}", endpoint);
                        self.client = None;
                    } else {
                        error!("UDP send error: {}", err);
                    }
                    break;
                }
            }
        }
    }

    async fn handle_router_message_from_host(&mut self, message: router::Message) {
        router::TO_GNC_CHAN.send(message).await;
    }

    async fn handle_command(&mut self, rx_message: host_to_mote::Message) {
        match rx_message {
            host_to_mote::Message::Ping => {
                info!("Parsed ping request, responding.");
                let _ = self.link.send(mote_to_host::Message::Pong);
            }
            host_to_mote::Message::Pong => {
                info!("Received ping response from host.");
            }
            host_to_mote::Message::SetDriveBaseVelocity(cmd) => {
                let _ = MOTOR_COMMAND_CHANNEL.try_send(cmd);
            }
            host_to_mote::Message::Router(message) => {
                self.handle_router_message_from_host(message).await;
            }
            _ => {
                error!("Received unhandled message type");
            }
        }
    }
}
