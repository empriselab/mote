# mote_link

Python client for talking to [Mote](https://empriselab.github.io/mote/) robots over the network.

`mote_link` handles discovery (via mDNS), connection, and message serialization / deserialization.

## Installation

```bash
pip install mote_link
```

## Example

```python
import asyncio

from mote_link.link import MoteClient, SetDriveBaseVelocity, DriveBaseState


async def main():
    async with MoteClient() as mote:
        # Opens an interactive prompt if the robot's IP or UID isn't known.
        await mote.connect()

        # Drive both wheels forward.
        await mote.send(SetDriveBaseVelocity(left_velocity_rad=1.0, right_velocity_rad=1.0))

        # Read messages from the robot.
        message = await mote.recv()
        if isinstance(message, DriveBaseState):
            print(f"left: {message.left}, right: {message.right}")


asyncio.run(main())
```

## Documentation

See the [Mote documentation](https://empriselab.github.io/mote/) for hardware setup, configuration,
and the full API reference.
