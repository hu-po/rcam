# Hardware

This code mostly runs on a small pc connected to a LAN box.

- `trossen-ai`: System76 Meerkat PC (13th Gen Intel i5-1340P, 16-core @ 4.6GHz) (15GB RAM)

Realsense cameras are connected via USB

- `realsense1`: Intel Realsense D405 (1MP RGBD)
- `realsense2`: Intel Realsense D405 (1MP RGBD)

Cameras connected to a LAN box via ethernet.

- `camera1`: Amcrest PoE cameras (5MP RGB)
- `camera2`: Amcrest PoE cameras (5MP RGB)
- `camera3`: Amcrest PoE cameras (5MP RGB)
- `camera4`: Amcrest PoE cameras (5MP RGB)
- `camera5`: Amcrest PoE cameras (5MP RGB)

There is also a Raspberry Pi that is acting as the NTP server using `chrony`.

- `rpi1`: Raspberry Pi 5 (4-core ARM Cortex-A76 @ 2.4 GHz) (8GB RAM)