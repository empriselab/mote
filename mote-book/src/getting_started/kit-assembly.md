# Kit Assembly

This tutorial will guide you through assembling your Mote. It should take around 15 minutes.

## Gather Parts

First, check that you have the required parts.
Missing something? Check out [the sourcing guide](../hardware-sourcing/acquire.md).

![Required parts](./assets/assembly/all_parts.webp)

### Custom Parts

| Item | Quantity | Picture | 
|---|---|---|
| Circuit Board | 1 | ![](./assets/assembly/circuit_board.webp) |
| Left Motor Mount | 1 | ![](./assets/assembly/left_wheel_mount.webp) |
| Right Motor Mount | 1 | ![](./assets/assembly/right_wheel_mount.webp) |
| Tail Runner | 1 | ![](./assets/assembly/tail_runner.webp) |
| LiDAR Mount | 1 | ![](./assets/assembly/lidar_stand.webp) |
| Wheel Spacers | 2 | ![](./assets/assembly/wheel_spacers.webp) |

### Off the Shelf Parts

| Item | Quantity | Picture | 
|---|---|---|
| LiDAR | 1 | ![](./assets/assembly/rp_c1_lidar.webp) |
| Orange Wheel | 2 | ![](./assets/assembly/wheels.webp) |
| Motor with Encoder | 2 | ![](./assets/assembly/motors.webp) |
| 25mm M2.5 Hex Head Machine Screws | 6 | ![](./assets/assembly/25mm_screws.webp) |
| 6mm M2.5 Hex Head Machine Screws | 2 | ![](./assets/assembly/6mm_screws.webp) |

### Miscellaneous Parts

| Item | Quantity | Picture | 
|---|---|---|
| 2mm Hex Key | 1 | ![](./assets/assembly/hex_key.webp) |
| USB-C to C Cable | 1 | TODO |
| 5000 mAh USB Portable Battery  | 1 | TODO |

## Assemble Your Mote

### Motor Mounts

![](./assets/assembly/1_assembly_right_motor.webp)

Place the motor into the bottom half of the motor mount. 
Ensure that the motor is seated in the pocket of the mount.

![](./assets/assembly/2_assembly_right_motor.webp)

Connect the top side of the motor mount to the bottom side, securing the motor.

![](./assets/assembly/3_assembly_right_motor.webp)

Repeat for the left side motor mount. 

![](./assets/assembly/4_assembly_both_motors.webp)

### LiDAR Stackup

Slide the circuit board between the two halves of the motor mounts, aligning the holes in the mounts with those in the circuit board.

![](./assets/assembly/5_assembly_motors_on_board.webp)
![](./assets/assembly/6_assembly_motors_on_board_three_quarters.webp)

Insert the 25mm screws through the holes in the motor mounts and circuit board.

![](./assets/assembly/7_assembly_screws_in_motor_mounts.webp)
![](./assets/assembly/8_assembly_screws_through_board.webp)

Place the LiDAR stand onto the 25mm screws. 

![](./assets/assembly/9_assembly_screws_through_lidar_stand.webp)

Place the LiDAR on top of the LiDAR stand, then thread the 25mm screws into the LiDARs base.

![](./assets/assembly/10_assembly_lidar_on_stack.webp)
![](./assets/assembly/11_assembly_lidar_tighten.webp)

### Cables

Using the motor cables, connect both motors to the circuit board.

> [!IMPORTANT]
> Be careful to connect each motor to it's corresponding socket. 
> Look for the "Motor" arrows on the silkscreen.
>
> <img src="./assets/assembly/pcb_motor_direction.png" width="35%"/>

<!-- ![](./assets/assembly/12_assembly_left_motor_cable.webp) -->
<!-- ![](./assets/assembly/13_assembly_left_motor_cable.webp) -->
![](./assets/assembly/14_assembly_right_motor_cable.webp)

Route the LiDAR's cable through the hole in the circuit board.

![](./assets/assembly/15_assembly_lidar_cable_through.webp)

Connect the LiDAR's cable to the connector on the bottom of the circuit board labled "LiDAR".

![](./assets/assembly/16_assembly_lidar_cable_connect.webp)

Restrain the motor cables using a cable tie. Trim the cable tie to prevent dragging.

![](./assets/assembly/17_assembly_ziptie.webp)
![](./assets/assembly/18_assembly_ziptie_trim.webp)

### Wheels

Add a wheel spacer to the left motor.

![](./assets/assembly/19_assembly_wheel_spacer.webp)

Push the wheel onto the motor shaft.

![](./assets/assembly/20_assembly_wheel.webp)

Secure the wheel with a 25mm screw.

> [!TIP]
> The M2.5 screw must be threaded into the hole on the wheel. You may need to apply some pressure to get the thread started.

![](./assets/assembly/21_assembly_wheel_with_screw.webp)

Repeat for the right wheel.

![](./assets/assembly/22_assembly_both_wheels.webp)

### Tail Runner

Place a 6mm screw into the hole on the tail runner. Align the screw with the nut on the circuit board, then use the hex hey to secure the tail runner to the circuit board. Repeat with the second 6mm screw and hole on the tail runner.

![](./assets/assembly/23_assembly_tail_screw.webp)

## Next Steps

You've successfully assembled your Mote! Move on to [configuration](./configuration.md) to get your robot ready to use.

![](./assets/assembly/24_assembly_complete.webp)
