import asyncio
import json
from typing import Union
from unittest.mock import MagicMock

import pytest

from mote_link.link import MoteClient, MoteError, MoteProtocolError
from mote_link._generated import (
    AuthOrRefused,
    Bit,
    BitCollection,
    BitResult,
    DriveBaseState,
    Err,
    ImuAxisTriple,
    ImuMeasurement,
    NetworkConnection,
    Ok,
    Other,
    Ping,
    Point,
    Pong,
    RequestNetworkScan,
    Scan,
    SetDriveBaseVelocity,
    SetNetworkConnectionConfig,
    SetUid,
    State,
    Timeout,
    WheelJointState,
    HostToMoteMessage,
    MoteToHostMessage,
    from_wire_json,
    to_wire_json,
)


class TestHostToMoteEncoding:
    def test_ping(self):
        assert json.loads(to_wire_json(Ping())) == "Ping"

    def test_pong(self):
        assert json.loads(to_wire_json(Pong())) == "Pong"

    def test_request_network_scan(self):
        assert json.loads(to_wire_json(RequestNetworkScan())) == "RequestNetworkScan"

    def test_set_network_connection_config(self):
        msg = SetNetworkConnectionConfig(ssid="MyNetwork", password="secret")
        assert json.loads(to_wire_json(msg)) == {
            "SetNetworkConnectionConfig": {"ssid": "MyNetwork", "password": "secret"}
        }

    def test_set_uid(self):
        assert json.loads(to_wire_json(SetUid(uid="mote-123"))) == {
            "SetUid": {"uid": "mote-123"}
        }

    def test_drive_base_command(self):
        msg = SetDriveBaseVelocity(
            left_velocity_rad_per_s=1.5, right_velocity_rad_per_s=-0.5
        )
        assert json.loads(to_wire_json(msg)) == {
            "SetDriveBaseVelocity": {
                "left_velocity_rad_per_s": 1.5,
                "right_velocity_rad_per_s": -0.5,
            }
        }


class TestRoundTrip:
    """Round-trips through to_wire_json/from_wire_json, mirroring what actually
    crosses the mote_ffi <-> Rust boundary."""

    def test_ping_round_trip(self):
        assert from_wire_json(to_wire_json(Ping()), HostToMoteMessage) == Ping()

    def test_pong_round_trip(self):
        assert from_wire_json(to_wire_json(Pong()), MoteToHostMessage) == Pong()

    def test_set_uid_round_trip(self):
        msg = SetUid(uid="mote-123")
        assert from_wire_json(to_wire_json(msg), HostToMoteMessage) == msg

    def test_set_network_connection_config_round_trip(self):
        msg = SetNetworkConnectionConfig(ssid="MyNetwork", password="secret")
        assert from_wire_json(to_wire_json(msg), HostToMoteMessage) == msg

    def test_set_drive_base_velocity_round_trip(self):
        msg = SetDriveBaseVelocity(
            left_velocity_rad_per_s=1.5, right_velocity_rad_per_s=-0.5
        )
        assert from_wire_json(to_wire_json(msg), HostToMoteMessage) == msg

    def test_scan_points_preserved(self):
        msg = Scan(
            value=[
                Point(quality=1, angle_rad=0.1, distance_mm=10.0),
                Point(quality=2, angle_rad=0.2, distance_mm=20.0),
            ]
        )
        result = from_wire_json(to_wire_json(msg), MoteToHostMessage)
        assert isinstance(result, Scan)
        assert len(result.value) == 2
        assert result.value[1].distance_mm == 20.0

    def test_drive_base_state(self):
        msg = DriveBaseState(
            left=WheelJointState(
                effort_percent=0.5, velocity_rad_per_s=1.0, position_rad=0.0
            ),
            right=WheelJointState(
                effort_percent=0.3, velocity_rad_per_s=0.8, position_rad=0.1
            ),
        )
        result = from_wire_json(to_wire_json(msg), MoteToHostMessage)
        assert result == msg

    def test_imu_measurement(self):
        msg = ImuMeasurement(
            accel=ImuAxisTriple(x=0.1, y=0.2, z=9.8),
            gyro=ImuAxisTriple(x=0.01, y=0.02, z=0.03),
        )
        result = from_wire_json(to_wire_json(msg), MoteToHostMessage)
        assert result == msg

    def test_state_round_trip_including_mac(self):
        state = State(
            uid="mote-1",
            ip="192.168.1.100",
            mac="aa:bb:cc:dd:ee:ff",
            current_network_connection=Ok(value="MyWifi"),
            available_network_connections=[
                NetworkConnection(ssid="MyWifi", strength=80)
            ],
            built_in_test=BitCollection(
                power=[Bit(name="battery", result=BitResult.PASS)],
                wifi=[],
                lidar=[Bit(name="lidar_init", result=BitResult.WAITING)],
                imu=[],
                encoders=[Bit(name="left_enc", result=BitResult.FAIL)],
            ),
        )
        result = from_wire_json(to_wire_json(state), MoteToHostMessage)
        assert result == state
        assert result.mac == "aa:bb:cc:dd:ee:ff"

    def test_state_with_no_network_connection(self):
        state = State(
            uid="mote-1",
            ip=None,
            mac=None,
            current_network_connection=None,
            available_network_connections=[],
            built_in_test=BitCollection(
                power=[], wifi=[], lidar=[], imu=[], encoders=[]
            ),
        )
        result = from_wire_json(to_wire_json(state), MoteToHostMessage)
        assert result == state


class TestConnectionErrorRoundTrip:
    """Covers `current_network_connection`'s nested union shape:
    `Option<Result<String, ConnectionError>>`."""

    RESULT = Union[Ok, Err]

    def test_connected(self):
        value = Ok(value="MyWifi")
        result = from_wire_json(to_wire_json(value), self.RESULT)
        assert result == value

    def test_timeout(self):
        value = Err(value=Timeout())
        result = from_wire_json(to_wire_json(value), self.RESULT)
        assert result == value

    def test_auth_or_refused(self):
        value = Err(value=AuthOrRefused())
        result = from_wire_json(to_wire_json(value), self.RESULT)
        assert result == value

    def test_other(self):
        value = Err(value=Other(value="radio hardware fault"))
        result = from_wire_json(to_wire_json(value), self.RESULT)
        assert result == value

    def test_embedded_in_state(self):
        state = State(
            uid="mote-1",
            ip=None,
            mac=None,
            current_network_connection=Err(value=Other(value="radio hardware fault")),
            available_network_connections=[],
            built_in_test=BitCollection(
                power=[], wifi=[], lidar=[], imu=[], encoders=[]
            ),
        )
        result = from_wire_json(to_wire_json(state), MoteToHostMessage)
        assert result.current_network_connection == Err(
            value=Other(value="radio hardware fault")
        )


def _run(coro):
    return asyncio.run(coro)


def _connected_client() -> tuple[MoteClient, MagicMock, MagicMock]:
    """A MoteClient wired up with mocks in place of a real connect().

    Returns the mocks directly rather than making the caller re-read
    `client._link`/`client._protocol`, which are `X | None`.
    """
    client = MoteClient()
    link_mock = MagicMock()
    protocol_mock = MagicMock()
    protocol_mock.transport = MagicMock()
    protocol_mock._queue = asyncio.Queue()
    client._link = link_mock
    client._protocol = protocol_mock
    return client, link_mock, protocol_mock


class TestMoteClientNotConnected:
    def test_send_without_connect_raises(self):
        with pytest.raises(MoteError):
            _run(MoteClient().send(Ping()))

    def test_recv_without_connect_raises(self):
        with pytest.raises(MoteError):
            _run(MoteClient().recv())


class TestMoteClientSend:
    def test_sends_wire_json_and_drains_transmit_queue(self):
        client, link_mock, protocol_mock = _connected_client()
        # First poll_transmit call returns a packet, second signals "nothing left".
        link_mock.poll_transmit.side_effect = [json.dumps([1, 2, 3]), None]

        _run(client.send(SetUid(uid="mote-abc")))

        link_mock.send.assert_called_once_with(to_wire_json(SetUid(uid="mote-abc")))
        protocol_mock.transport.sendto.assert_called_once_with(bytes([1, 2, 3]))


class TestMoteClientRecv:
    def test_decodes_a_complete_message(self):
        client, link_mock, protocol_mock = _connected_client()
        protocol_mock._queue.put_nowait(b"raw-packet-bytes")
        link_mock.poll_receive.return_value = to_wire_json(Pong())

        result = _run(client.recv())

        assert result == Pong()
        link_mock.handle_receive.assert_called_once()

    def test_skips_packets_the_link_cant_decode(self):
        client, link_mock, protocol_mock = _connected_client()
        protocol_mock._queue.put_nowait(b"bad-packet")
        protocol_mock._queue.put_nowait(b"good-packet")
        link_mock.poll_receive.side_effect = [
            ValueError("bad frame"),
            to_wire_json(Ping()),
        ]

        result = _run(client.recv())

        assert result == Ping()

    def test_wire_json_the_link_accepted_but_generated_types_reject_raises_protocol_error(
        self,
    ):
        client, link_mock, protocol_mock = _connected_client()
        protocol_mock._queue.put_nowait(b"raw-packet-bytes")
        # A tag no MoteToHostMessage member has -- e.g. mote-api sent a newer
        # message variant this build of mote_link doesn't know about yet.
        link_mock.poll_receive.return_value = json.dumps({"SomeFutureVariant": {}})

        with pytest.raises(MoteProtocolError):
            _run(client.recv())


class TestMoteClientClose:
    def test_close_closes_the_transport(self):
        client, _, protocol_mock = _connected_client()

        _run(client.close())

        protocol_mock.transport.close.assert_called_once()
        assert client._link is None
        assert client._protocol is None

    def test_aexit_calls_close(self):
        client, _, protocol_mock = _connected_client()

        _run(client.__aexit__(None, None, None))

        protocol_mock.transport.close.assert_called_once()
