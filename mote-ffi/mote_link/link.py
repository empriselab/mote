from __future__ import annotations

# PyO3 does not support exporting type stubs for generated modules
# https://github.com/PyO3/maturin/pull/2940
import mote_link.mote_ffi as mote_ffi  # ty:ignore[unresolved-import]

import asyncio
import ipaddress
import json
import logging
import socket

from mote_link._generated import (
    HostToMoteMessage,
    MoteToHostMessage,
    from_wire_json,
    to_wire_json,
)


UDP_PORT = 7475

_logger = logging.getLogger(__name__)


class MoteError(Exception):
    """Base class for all errors raised by mote_link."""


class MoteConnectionError(MoteError):
    """Raised when a connection attempt to Mote fails."""


class MoteProtocolError(MoteError):
    """Raised when a message received from Mote can't be decoded."""


# Prompt the client to chose a robot from all devices advertising on the provided service.
# Scans for `service_name` via mDNS for 3 seconds, then either auto-connects if only one
# device is found, or presents a selection prompt if multiple devices are found.
# Raises MoteConnectionError if no devices are found.
async def _chose_from_mdns_service(service_name: str) -> str:
    from zeroconf import ServiceStateChange
    from zeroconf.asyncio import AsyncServiceBrowser, AsyncServiceInfo, AsyncZeroconf
    import survey

    found: dict[str, str] = {}  # service instance name -> IPv4 string

    def on_change(zeroconf, service_type, name, state_change):
        if state_change == ServiceStateChange.Added:
            asyncio.ensure_future(_fetch(zeroconf, service_type, name))

    async def _fetch(zeroconf, service_type, name):
        info = AsyncServiceInfo(service_type, name)
        await info.async_request(zeroconf, 3000)
        ipv4 = next((a for a in info.addresses if len(a) == 4), None)
        if ipv4 is not None:
            ip = socket.inet_ntoa(ipv4)
            found[name] = ip
            print(f"  Found: {name} at {ip}")

    azc = AsyncZeroconf()
    browser = AsyncServiceBrowser(azc.zeroconf, service_name, handlers=[on_change])

    print("Scanning for Motes...")
    await asyncio.sleep(20.0)

    await browser.async_cancel()
    await azc.async_close()

    if not found:
        raise MoteConnectionError("No Motes found on the network.")

    devices = list(found.items())

    if len(devices) == 1:
        name, ip = devices[0]
        print(f"Connecting to {name} at {ip}")
        return ip

    try:
        idx = survey.routines.select(
            "Select a Mote: ",
            options=[f"{name} ({ip})" for name, ip in devices],
        )
    except KeyboardInterrupt:
        raise SystemExit(130) from None
    return devices[idx][1]


# Simple protocol for dumping byting onto / reading bytes from a queue
class _MoteProtocol(asyncio.DatagramProtocol):
    def __init__(self):
        self.transport: asyncio.DatagramTransport | None = None
        self._queue: asyncio.Queue[bytes] = asyncio.Queue[bytes]()

    def connection_made(self, transport):
        self.transport = transport

    def datagram_received(self, data, addr):
        self._queue.put_nowait(data)

    def error_received(self, exc):
        print(f"UDP error: {exc}")

    def connection_lost(self, exc):
        pass


class MoteClient:
    def __init__(self):
        """
        Create a new Mote client.
        """
        self.ip = None
        self._protocol: _MoteProtocol | None = None
        self._link: mote_ffi.Link | None = None

    async def __aenter__(self):
        return self

    async def _open_connection(self):
        loop = asyncio.get_event_loop()
        _, self._protocol = await loop.create_datagram_endpoint(
            _MoteProtocol,
            remote_addr=(str(self.ip), UDP_PORT),
        )
        self._link = mote_ffi.Link()

    async def connect(self):
        """
        Connect to Mote.

        This method will open an interactive discovery prompt.
        Use this method if you do not know the ip or unique ID of your robot and your network supports MDNS.
        """
        try:
            self.ip = await _chose_from_mdns_service("_mote-api._udp.local.")
            await self._open_connection()
        except MoteConnectionError:
            try:
                ip_str = input(
                    "Could not find Motes using autodiscovery. Enter Mote IP address (x.x.x.x): "
                )
            except (KeyboardInterrupt, EOFError):
                raise SystemExit(130) from None
            await self.connect_with_ip(ipaddress.IPv4Address(ip_str.strip()))

    async def connect_with_uid(self, uid: str):
        """
        Connect to Mote.

        Use this method if you know the unique ID of your robot, and your network / OS support MDNS.
        """
        hostname = f"{uid}.local"
        print(f"Attempting to connect to {hostname}...")
        try:
            self.ip = socket.gethostbyname(hostname)
        except socket.error as e:
            raise MoteConnectionError(
                f"Could not resolve {hostname}: {e}. "
                "Check the UID, ensure Mote is on the network, and that mDNS is supported. "
                "If you know the IP address, use connect_with_ip instead."
            ) from e
        print(f"Resolved {hostname} to {self.ip}")
        await self._open_connection()

    async def connect_with_ip(self, ip: ipaddress.IPv4Address):
        """
        Connect to Mote.

        Use this method if you know the IP of you robot.
        If your network does not support MDNS you must use this method.
        You can find your robots IP by connecting using USB and visiting [the configuration page](https://empriselab.github.io/mote/configuration/).
        """
        self.ip = ip
        await self._open_connection()

    async def close(self):
        """
        Disconnect from Mote.

        Only needed if the client wasn't opened as an `async with` context
        manager; `__aexit__` already calls this.
        """
        if self._protocol is not None and self._protocol.transport is not None:
            self._protocol.transport.close()
        self._protocol = None
        self._link = None

    async def send(self, message: HostToMoteMessage):
        """
        Send a message to Mote.
        """
        if (
            self._link is None
            or self._protocol is None
            or self._protocol.transport is None
        ):
            raise MoteError("Not connected, try calling MoteClient.connect")

        self._link.send(to_wire_json(message))
        while True:
            transmit_json = self._link.poll_transmit()
            if transmit_json is None:
                break
            self._protocol.transport.sendto(bytes(json.loads(transmit_json)))

    async def recv(self) -> MoteToHostMessage:
        """
        Receive one message from Mote.

        Suspends until a complete message is decoded, yielding control to the
        event loop between packets.
        """
        if self._link is None or self._protocol is None:
            raise MoteError("Not connected, try calling MoteClient.connect")

        while True:
            data = await self._protocol._queue.get()
            self._link.handle_receive(json.dumps(list(data)))
            try:
                message_json = self._link.poll_receive()
            except ValueError as e:
                _logger.warning("Discarded undecodable message from Mote: %s", e)
                continue
            if message_json is not None:
                try:
                    return from_wire_json(message_json, MoteToHostMessage)
                except (ValueError, KeyError) as e:
                    raise MoteProtocolError(
                        f"Could not decode message from Mote: {e}"
                    ) from e

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close()
