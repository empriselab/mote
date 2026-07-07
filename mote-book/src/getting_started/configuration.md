# Configuration

This page will guide you through setting up your Mote for the first time.

## Flashing Firmware

Before you can use your Mote, you must program the microcontroller on board the robot.

1. Download the latest version of mote-firmware: [vX.X.X](TODO).

2. While holding down the "BOOT" button on your Mote, connect the robot to your computer using the USB-C connector. Mote will appear as a USB drive. 

3. Drag and drop the mote-firmware UF2 file into the Mote USB drive. The drive will disappear a couple of seconds after dropping in the file.

## Connecting to WiFi

Mote communicates using WiFi. In order to work with Mote, we will log it onto a WiFi network.

1. Open [the Mote configuration page](../configuration). Click `[ connect ]` and select "Mote Serial".

2. Locate your WiFi network under "Detected Networks". Click `[ connect ]` and enter your WiFi password. If you are using a public network, leave this field blank.

3. Press enter.

4. Wait 30 seconds for Mote to connect to the network. When the robot has successfully connected, you will see "currently connected" next to your WiFi in the detected networks list.

5. Under "Identification", note your Mote's IP. You will need the IP to establish communication with your robot in the next section.

## Give Your Mote a Name

You can use a friendly name to communicate with your Mote.

1. If it is not already open, open [the Mote configuration page](../configuration). Click `[ connect ]` and select "Mote Serial".

2. Next to "Unique Identifier", click `[ update ]`. 

3. Enter your Mote's name in the box. 

4. Press enter.

> [!IMPORTANT]
> Only one Mote can have a given name on a single network. If you expect other Motes to be used on the same network, choose a name that is unique enough to prevent conflicts.

## Next Steps

Your Mote is now configured to communicate on your local network. [The Hello World guide](./hello-world.md) will show you how to use your robot.

## Troubleshooting

Having issues? Check out [the trouble shooting guide](../troubleshooting/common_issues.md).

