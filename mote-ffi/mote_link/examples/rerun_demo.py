import asyncio
import colorsys
import math

import pyglet
import rerun as rr
import rerun.blueprint as rrb
from pyglet.window import key

from mote_link import (
    DriveBaseState,
    ImuMeasurement,
    MoteClient,
    MoteConnectionError,
    Ping,
    Pong,
    Scan,
    SetDriveBaseVelocity,
    State,
)


def _log_drive_base_state(state: DriveBaseState):
    for side, wheel in [("left", state.left), ("right", state.right)]:
        rr.log(f"drive_base/{side}/effort_percent", rr.Scalars(wheel.effort_percent))
        rr.log(
            f"drive_base/{side}/velocity_rad_per_s",
            rr.Scalars(wheel.velocity_rad_per_s),
        )
        rr.log(f"drive_base/{side}/position_rad", rr.Scalars(wheel.position_rad))


def _log_imu_measurement(imu: ImuMeasurement):
    rr.log("imu/accel/x", rr.Scalars(imu.accel.x))
    rr.log("imu/accel/y", rr.Scalars(imu.accel.y))
    rr.log("imu/accel/z", rr.Scalars(imu.accel.z))
    rr.log("imu/gyro/x", rr.Scalars(imu.gyro.x))
    rr.log("imu/gyro/y", rr.Scalars(imu.gyro.y))
    rr.log("imu/gyro/z", rr.Scalars(imu.gyro.z))


def _log_scan(scan: Scan):
    positions = [
        [
            math.cos(p.angle_rad) * p.distance_mm,
            math.sin(p.angle_rad) * p.distance_mm,
        ]
        for p in scan.value
    ]
    colors = []
    for p in scan.value:
        h = (p.distance_mm / (20.0 * 360.0)) % 1.0
        r, g, b = colorsys.hsv_to_rgb(h, 1.0, 1.0)
        colors.append([int(r * 255), int(g * 255), int(b * 255)])

    rr.log(
        "lidar_scan",
        rr.Points2D(positions, colors=colors, radii=10.0),
    )


class _Joystick:
    """A small pyglet window showing a draggable virtual joystick.

    Drag the knob with the mouse or, while the window is focused, hold the arrow
    keys to set a velocity vector inside the unit disc.
    """

    _SIZE = 300
    _CENTER = 150
    _RADIUS = 110
    _KNOB_RADIUS = 30

    def __init__(self):
        self.quit = asyncio.Event()
        self._dragging = False
        self._offset = [0.0, 0.0]

        self._window = pyglet.window.Window(
            self._SIZE, self._SIZE, caption="Mote teleop"
        )
        self._window.set_vsync(False)

        self._batch = pyglet.graphics.Batch()

        self._base = pyglet.shapes.Circle(
            self._CENTER,
            self._CENTER,
            self._RADIUS,
            color=(38, 40, 52),
            batch=self._batch,
        )
        self._ring = pyglet.shapes.Arc(
            self._CENTER,
            self._CENTER,
            self._RADIUS,
            thickness=3,
            color=(120, 130, 165),
            batch=self._batch,
        )
        self._knob = pyglet.shapes.Circle(
            self._CENTER,
            self._CENTER,
            self._KNOB_RADIUS,
            color=(205, 210, 225),
            batch=self._batch,
        )
        self._label = pyglet.text.Label(
            "Drag or arrow keys · Esc quits",
            x=self._CENTER,
            y=16,
            anchor_x="center",
            font_size=10,
            color=(200, 200, 210, 255),
            batch=self._batch,
        )

        self._keys = key.KeyStateHandler()
        self._window.push_handlers(self._keys)
        self._window.push_handlers(self)

    def on_mouse_press(self, x, y, button, modifiers):
        self._dragging = True
        self._set_offset(x - self._CENTER, y - self._CENTER)

    def on_mouse_drag(self, x, y, dx, dy, buttons, modifiers):
        if self._dragging:
            self._set_offset(x - self._CENTER, y - self._CENTER)

    def on_mouse_release(self, x, y, button, modifiers):
        self._dragging = False
        self._offset = [0.0, 0.0]

    def on_key_press(self, symbol, modifiers):
        if symbol == key.ESCAPE:
            self.quit.set()

    def on_close(self):
        self.quit.set()

    def _set_offset(self, off_x, off_y):
        mag = math.hypot(off_x, off_y)
        if mag > self._RADIUS:
            off_x = off_x / mag * self._RADIUS
            off_y = off_y / mag * self._RADIUS
        self._offset = [off_x, off_y]

    def read(self):
        """Pump window events, render a frame, and return the (x, y) input.

        Both components are in [-1, 1]: x is turn (right positive), y is forward
        (up positive).
        """
        self._window.switch_to()
        self._window.dispatch_events()  # fires the on_* handlers above

        if not self._dragging:
            key_x = self._keys[key.RIGHT] - self._keys[key.LEFT]
            key_y = self._keys[key.UP] - self._keys[key.DOWN]
            self._set_offset(key_x * self._RADIUS, key_y * self._RADIUS)

        self._knob.position = (
            self._CENTER + self._offset[0],
            self._CENTER + self._offset[1],
        )
        self._window.clear()
        self._batch.draw()
        self._window.flip()

        return self._offset[0] / self._RADIUS, self._offset[1] / self._RADIUS

    def close(self):
        self._window.close()


# Example application that connects to Mote and logs sensor data to rerun.
async def run_main():
    rr.init("mote_rerun_example_python")
    server_uri = rr.serve_grpc()
    rr.serve_web_viewer(connect_to=server_uri)

    wheel_time_ranges = [
        rrb.VisibleTimeRange(
            "log_time",
            start=rrb.TimeRangeBoundary.cursor_relative(seconds=-10.0),
            end=rrb.TimeRangeBoundary.cursor_relative(seconds=5.0),
        )
    ]

    # (row label, {side: [entity paths]})
    wheel_signal_rows = [
        (
            "Velocity",
            {
                side: [
                    f"+ /drive_base/{side}/velocity_rad_per_s",
                    f"+ /drive_base/{side}/velocity_command_rad_per_s",
                ]
                for side in ("left", "right")
            },
        ),
        (
            "Effort",
            {
                side: [f"+ /drive_base/{side}/effort_percent"]
                for side in ("left", "right")
            },
        ),
        (
            "Position",
            {
                side: [f"+ /drive_base/{side}/position_rad"]
                for side in ("left", "right")
            },
        ),
    ]

    wheel_rows = [
        rrb.Horizontal(
            *[
                rrb.TimeSeriesView(
                    name=f"{side.capitalize()} Wheel {label}",
                    contents=paths,
                    time_ranges=wheel_time_ranges,
                )
                for side, paths in signals.items()
            ]
        )
        for label, signals in wheel_signal_rows
    ]

    blueprint = rrb.Blueprint(
        rrb.Horizontal(
            rrb.Spatial2DView(
                name="LiDAR",
                origin="/lidar_scan",
                visual_bounds=rrb.VisualBounds2D(
                    x_range=[-7000, 7000], y_range=[-7000, 7000]
                ),
                time_ranges=[
                    rrb.VisibleTimeRange(
                        "log_time",
                        start=rrb.TimeRangeBoundary.cursor_relative(seconds=-0.2),
                        end=rrb.TimeRangeBoundary.cursor_relative(),
                    )
                ],
            ),
            rrb.Vertical(
                rrb.TimeSeriesView(name="Accel", origin="/imu/accel"),
                rrb.TimeSeriesView(name="Gyro", origin="/imu/gyro"),
                *wheel_rows,
            ),
        ),
        rrb.SelectionPanel(state="collapsed"),
        rrb.TimePanel(state="collapsed"),
    )
    rr.send_blueprint(blueprint)

    async with MoteClient() as client:
        await client.connect()

        print("Pinging Mote")
        await client.send(Ping())

        print("Drag the joystick knob or use the arrow keys (window must be focused).")
        print("Press Esc or close the window to quit.")

        joystick = _Joystick()

        async def recv_loop():
            while True:
                message = await client.recv()

                if isinstance(message, Pong):
                    print("Got pong from Mote.")
                elif isinstance(message, Ping):
                    print("Mote pinged host.")
                    await client.send(Pong())
                elif isinstance(message, Scan):
                    _log_scan(message)
                elif isinstance(message, DriveBaseState):
                    _log_drive_base_state(message)
                elif isinstance(message, ImuMeasurement):
                    _log_imu_measurement(message)
                elif isinstance(message, State):
                    print(f"Got system state {message}")

        async def command_loop():
            speed = 10.0  # rad/s, forward/back
            turn = 4.0  # rad/s, differential turn component
            dt = 0.05  # 20 Hz command rate
            while not joystick.quit.is_set():
                turn_x, forward_y = joystick.read()
                left = forward_y * speed + turn_x * turn
                right = forward_y * speed - turn_x * turn

                await client.send(
                    SetDriveBaseVelocity(
                        left_velocity_rad_per_s=left,
                        right_velocity_rad_per_s=right,
                    )
                )
                rr.log("drive_base/left/velocity_command_rad_per_s", rr.Scalars(left))
                rr.log("drive_base/right/velocity_command_rad_per_s", rr.Scalars(right))

                await asyncio.sleep(dt)

        recv_task = asyncio.create_task(recv_loop())
        try:
            await command_loop()
        finally:
            recv_task.cancel()
            await asyncio.gather(recv_task, return_exceptions=True)
            # Make sure the rover stops when we exit.
            await client.send(
                SetDriveBaseVelocity(
                    left_velocity_rad_per_s=0.0, right_velocity_rad_per_s=0.0
                )
            )
            joystick.close()


def main():
    try:
        asyncio.run(run_main())
    except KeyboardInterrupt:
        print("\nDisconnected.")
    except MoteConnectionError as e:
        print(f"Connection failed: {e}")
        raise SystemExit(1)


if __name__ == "__main__":
    main()
