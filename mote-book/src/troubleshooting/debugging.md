# Debugging

## Status LEDs

The underside of Mote has three status LEDs indicating system faults.

### PWR LED

The power LED indicates if Mote is receiving sufficient voltage and current to operate.
Mote requires a 5V (USB standard) power supply that can deliver at least 1.5A.

* 🟥 Not receiving sufficient power. 
    * The power supply cannot deliver 1.5A. 
    * LiDAR and motors are disabled.
    * Check that the USB power bank you are using is capable of delivering > 1.5A.
* 🟨 Cannot read power supply current capacity. 
    * This is a firmware or hardware fault.
    * LiDAR and motors are disabled.
* 🟩 Nominal. Mote has sufficient power.

### WiFi LED

The WiFi LED indicates if WiFi is enabled and connected to a network.

* 🟥 WiFi has failed to initialize or is disabled.
    * If the PWR LED is yellow or red, Mote does not have enough power to enable WiFi.
    * If the PWR LED is green, this is a hardware or firmware fault.
* 🟨 WiFi has initialized but has not yet connected to a network.
    * Reference [getting started - configuration](../getting_started/configuration.html#connecting-to-wifi) to connect your Mote to a network.
* 🟩 Nominal. WiFi is configured and connected to a network.

### Err LED

The error LED indicates if the system's sensors are powered on and communicating correctly.

* 🟥 IMU fault.
    * Power cycle the robot.
* 🟪 LiDAR fault.
    * Check that the LiDAR's cable is securely seated.
    * Power cycle the robot.
* 🟩 Nominal. All sensors are operating as expected.


