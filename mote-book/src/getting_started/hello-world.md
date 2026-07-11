# Hello World

In this example we'll use the mote-link Python library to stream data from and send commands to your robot.

* A Mote
* A personal computer running any major operating system (MacOS, Windows, Linux)

<iframe src="https://app.rerun.io/version/0.34.1/index.html?url=https://empriselab.github.io/mote/getting_started/assets/hello_world/mote-link-example.rrd" width="100%" height="500px"></iframe>

## Setup

This example uses [uv](https://docs.astral.sh/uv/) to run a Python program. Install uv using the instructions in the [uv docs](https://docs.astral.sh/uv/getting-started/installation/).

Attach the battery to your Mote. Place your Mote on the ground with enough space for it to move around.

## Running the mote-link Demo

In the terminal of your choice, run the demo.

```bash
uvx --from mote-link rerun-demo
```

Follow the instructions in your terminal to select and connect to your Mote. 
Your browser will open to a dashboard showing realtime LiDAR, wheel encoder, accelerometer, and gyroscope data.
Use the arrow keys to drive Mote around.

## Next Steps

* [Python guide]() - learn how to write your own Python scripts for interfacing with Mote.
* [The Rust guide]() - like the Python guide, but using Rust 🦀.
* [The ROS 2 guide]() - learn how to use Mote with the Robot Operating System.

## Troubleshooting

### My Mote drives backwards

[Check your motor cable connections](./kit-assembly.html#cables). Are the left and right motor connections swapped?
