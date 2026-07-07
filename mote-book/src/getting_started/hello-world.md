# Hello World

In this example we'll use the mote-link Python library to remote control your Mote and view data streamed from the robot. You'll need:

* A Mote
* A personal computer running any major operating system (MacOS, Windows, Linux)

## Setup

This example uses [uv](https://docs.astral.sh/uv/) to run a Python program. Install uv using the instructions in the [uv docs](https://docs.astral.sh/uv/getting-started/installation/).

## Running the mote-link Demo

In the terminal of your choice, run the demo.

```bash
uvx mote_link
```

Follow the instructions in your terminal to select and connect to your Mote. 
Your browser will open to a dashboard showing realtime LiDAR, wheel encoder, accelerometer, and gyroscope data.

## Next Steps

The "Guides" section contains tutorials for working with Mote using various SDKs.

* [The Python guide]() will teach you how to write your own Python scripts for interfacing with Mote.
* [The Rust guide]() is like the Python guide, but using Rust 🦀.
* [The ROS 2 guide]() teaches how to use Mote with the Robot Operating System.

## Troubleshooting

Having issues? Check out [the trouble shooting guide](../troubleshooting/common_issues.md).
