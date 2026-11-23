# Hardware

The boards the platform supports, each with its board feature, e.g. `board-wave-esp32s3-lcd-147`, and an example crate
in `boards/`, and the SD cards which have been tried in them. What a board feature drives, and the optional features
it offers, are described in the README's [Board Capabilities](../README.md#board-capabilities).

## Boards

| Board | Feature | Chip | PSRAM | Display | SD card slot | Tested |
|---|---|---|---|---|---|---|
| [Waveshare ESP32-S3-LCD-1.47B](#waveshare-esp32-s3-lcd-147b) | `board-wave-esp32s3-lcd-147` | ESP32-S3R8, 16 MB flash | 8 MB | 1.47" colour, 172 × 320 | SDMMC, 4 lines | Yes, daily, as a capture device |
| [Waveshare ESP32-S3-RLCD-4.2](#waveshare-esp32-s3-rlcd-42) | `board-wave-esp32s3-rlcd-42` | ESP32-S3, N16R8 module | 8 MB | 4.2" reflective, black and white, 400 × 300 | SDMMC, 1 line | Yes, daily, as a capture device |
| [Guition JC3248W535C](#guition-jc3248w535c) | `board-guition-jc3248w535c` | ESP32-S3, N16R8 | 8 MB | 3.5" colour, 320 × 480, touch | SPI | Yes, daily, as a capture device |
| [Waveshare ESP32-C6-Touch-LCD-1.47](#waveshare-esp32-c6-touch-lcd-147) | `board-wave-esp32c6-touch-147` | ESP32-C6, one core | None | 1.47" colour, 172 × 320, touch | SPI, shared with the display | Yes, daily, as a capture device |
| [Waveshare ESP32-C6-LCD-1.47](#waveshare-esp32-c6-lcd-147) | `board-wave-esp32c6-lcd-147` | ESP32-C6, one core | None | 1.47" colour, 172 × 320 | Not driven | Its example |
| [CYD, ESP32-2432S028R](#cyd-esp32-2432s028r) | `board-cyd-esp32-28` | ESP32 | None | 2.8" colour, 240 × 320 | SPI | Its example; retired as a capture device |
| [Seeed Studio XIAO ESP32-C6](#seeed-studio-xiao-esp32-c6) | `board-seeed-esp32c6-xiao` | ESP32-C6 | None | None | None | Its example |
| [ESP32 DevKit V1](#esp32-devkit-v1) | `board-esp32-devkit-v1` | ESP32 | None | None | None | Its example |
| [ESP32-C3 SuperMini](#esp32-c3-supermini) | `board-esp32c3-supermini` | ESP32-C3 | None | None | None | Its example |

A board without PSRAM has only the chip's own RAM, about 300 KB free once Wi-Fi is running, which an application with
an SD card's file system, HTTPS and its own data quickly runs out of: the CYD was retired as a capture device on 12 Nov
2026 after its store ran it out of memory as it started, over and over. An ESP32-S3 with PSRAM, which the platform adds
to the heap, has room to spare.

### Waveshare ESP32-S3-LCD-1.47B

The [Waveshare ESP32-S3-LCD-1.47B](https://www.waveshare.com/product/esp32-related/boards-kits/esp32-s3/esp32-s3-lcd-1.47b.htm),
the ESP32-S3 sibling of the ESP32-C6-LCD-1.47, with the same display, on a smaller board. Its ESP32-S3R8 has two cores
and 8 MB of PSRAM, with 16 MB of flash. It comes with:

- a 1.47-inch colour LCD, 172 × 320, with an ST7789 controller, driven as the C6 board's is;
- a microSD card slot, wired for SDMMC with four data lines, whose pins are not on the header, so the board feature
  turns on `sd-card` itself. The slot is powered whenever the board is, so a restart which does not cut the power
  leaves the card as it was, which some cards do not recover from, see [SD Cards](#sd-cards);
- a QMI8658 6-axis IMU, on an I2C bus of its own (GPIO48 SDA, 47 SCL), which more devices can join, using
  `platform.i2c()`. Its driver needs the `tilt` or `ahrs` feature, which the example sets;
- a battery connector, with charging, whose voltage the platform reads on GPIO1, see `platform.battery()`;
- a WS2812 RGB LED, on GPIO38, given to the project in the `Remainder` as `rgb_led`, for `smart_led::new` with one LED;
- BOOT and RESET buttons. BOOT, GPIO0, can be used as a button once the board has started (the `boot-button` feature);
- a 2 × 9 pin header, with VBUS, BAT, 3V3, three grounds, and GPIO 0, 2 to 11, 43 and 44. GPIO 0 (unless
  `boot-button` takes it) and 2 to 11 are free, in the `Remainder`, with I2C1 and SPI3. 43 and 44 are the console's
  UART, which the platform keeps. The USB port is the chip's own, on GPIO19 and 20.

The example, in `boards/wave-esp32s3-lcd-147`, shows an analog clock, and a binary clock on a strip of 64 WS2812 LEDs
on GPIO5, as the C6 board's example does.

### Waveshare ESP32-S3-RLCD-4.2

The [Waveshare ESP32-S3-RLCD-4.2](https://www.waveshare.com/product/esp32-related/boards-kits/esp32-s3/esp32-s3-rlcd-4.2.htm),
whose ESP32-S3-WROOM-1-N16R8 module has two cores, 16 MB of flash and 8 MB of PSRAM. It comes with:

- a 4.2-inch reflective LCD, 400 × 300, black and white, with an ST7305 controller, and no backlight: it is lit by the
  room, like e-paper, and readable in daylight. The outermost row and column of pixels on each edge are hidden by its
  frame, so the platform offers the 398 × 298 which can be seen. The example's **Edge Test** *Feature* draws a pattern
  to check this;
- a microSD card slot, wired for SDMMC with one data line (the `sd-card` feature);
- a KEY button (the `key-button` feature), and a BOOT button which, once the board has started, can be used as a button
  too (the `boot-button` feature);
- an SHTC3 temperature and humidity sensor, which the platform always drives;
- an ES8311 audio codec and amplifier, with a connector for the speaker which comes with it, which the platform always
  drives;
- a PCF85063 real time clock, and two microphones, which the platform does not drive yet;
- a 2 × 8 pin, 2.54 mm header, with 3V3, VBUS, two grounds, and GPIO 0, 1, 2, 3, 17, 18, 19, 20, 43, 44, 13 and 14.
  GPIO 1, 2, 3 and 17 are free, in the `Remainder`. 13 and 14 are the board's I2C bus, which more devices can join,
  using `platform.i2c()`. 0 and 18 are the buttons, 19 and 20 the USB port, and 43 and 44 the console's UART, which the
  platform keeps;
- a holder for an 18650 lithium battery, with charging, and a PWR button which switches the battery's power. The
  battery is not supplied: an unprotected 18650, flat or button top, about 65 mm long, fits.

There is no reset button: unplug it, use the config page's **Restart** command, or reset it over USB. The example, in
`boards/wave-esp32s3-rlcd-42`, plays `sounds/Startup.wav` as it starts, and shows an analog clock, the SD card's size
and free space when KEY is pressed, and the temperature and humidity when BOOT is pressed, each with a beep.

### Guition JC3248W535C

An ESP32-S3 N16R8, with 16 MB of flash and 8 MB of PSRAM, and a 3.5-inch colour display, 320 × 480, with an AXS15231B
controller on QSPI and its touch screen. Its microSD slot is on an SPI bus of its own, SPI3, on GPIO12 (CLK), 11 (MOSI),
13 (MISO) and 10 (CS). The example, in `boards/guition-jc3248w535c`, shows an analog clock, a test pattern to enable on
the config page, which shows which way up the display is and that its colours are right, and a dot where the screen is
touched; BOOT shows the SD card's size and free space, or why there is no card.

### Waveshare ESP32-C6-Touch-LCD-1.47

The ESP32-C6-LCD-1.47's display, with an AXS5106L touch screen and a QMI8658 IMU, on an ESP32-C6, which has one core
and no PSRAM. Its microSD slot shares the display's SPI bus, with its chip select on GPIO4 (the `sd-card` feature).

### Waveshare ESP32-C6-LCD-1.47

An ESP32-C6 with the 1.47-inch ST7789 display, 172 × 320, without touch. The platform drives the display; it does not
drive the board's microSD slot.

### CYD, ESP32-2432S028R

The so-called "Cheap Yellow Display", whose ESP32-WROOM-32 module has no PSRAM, with a 2.8-inch TFT touch screen, a
microSD slot, on SPI3, an RGB LED and a light sensor (LDR). The platform drives the display, the slot and the LED, not
the touch screen or the LDR yet. There are clones with slight differences; the one developed on came from
[AliExpress](https://www.aliexpress.com/item/1005008229897039.html). [Random Nerd
Tutorials](https://randomnerdtutorials.com/?s=CYD) has useful information about it.

### Seeed Studio XIAO ESP32-C6

The [Seeed Studio XIAO ESP32-C6](https://www.seeedstudio.com/Seeed-Studio-XIAO-ESP32C6-p-5884.html), a small board
with 2.4 GHz Wi-Fi 6, Bluetooth 5 (LE) and IEEE 802.15.4 radios, and a status LED, which the platform drives.

### ESP32 DevKit V1

The common ESP32 development board, with a status LED; its example drives a strip of WS2812 LEDs with SPI, as the
ESP32's RMT is unreliable for them.

### ESP32-C3 SuperMini

A small ESP32-C3 board, with a status LED; like the DevKit V1's, its example drives WS2812 LEDs with SPI.

## SD Cards

The platform mounts a board's card once, as it starts, with FAT, see the README's [SD Cards](../README.md#sd-cards).
Cards are formatted FAT32, whatever their size, e.g. by macOS's Disk Utility as MS-DOS (FAT).

On every board here the card's slot is powered whenever the board is, so a restart which does not cut the power, by the
reset button, over USB, e.g. when flashing, or after a crash, leaves the card in whatever state it was in. The SD
standard only promises a card is reset when its power is cut; some cards then do not answer at all until the board is
unplugged, however often they are tried. The firmware's own restarts, e.g. the config page's **Restart**, unmount the
card first, see `crate::restart`, so that it is not in the middle of a write.

| Card | Where from | Tried in | Result |
|---|---|---|---|
| SanDisk Extreme A2, 128 GB, red and gold | | CYD, Waveshare ESP32-C6-Touch-LCD-1.47, ESP32-S3-LCD-1.47B | Works, and mounts after a restart by the reset button or over USB |
| SanDisk "Original", 32 GB, Class 10 UHS-I | [AliExpress](https://www.aliexpress.com/item/1005007535345003.html) | The other capture devices, and the ESP32-C6-Touch-LCD-1.47 | Works; not yet tried in the ESP32-S3-LCD-1.47B |
| "SanDisk" "SD-Red Gray-64GB-B", sold as "High Speed Micro SD Card U3 V30 A2", about £3, so probably not genuine | [AliExpress](https://www.aliexpress.com/item/1005012898627019.html) | ESP32-S3-LCD-1.47B | Not recommended: mounts after a power-on, but often not after a restart which does not cut the power, until the board is unplugged (`sdmmc_init_ocr: send_op_cond (1) returned 0x107`, a timeout) |

On 23 November 2026 the 64 GB card in the ESP32-S3-LCD-1.47B also had about 22 KB of a file garbled, with bits flipped
all through it, which was written over three minutes; whether the card or the board was at fault is not known, but
the card is probably counterfeit. Cards sold far below a brand's price, on marketplaces, often are: their controllers
are not the brand's, and some report more space than they have, so test a new card's capacity, e.g. with F3 or H2testw,
before trusting it.
bruce-db, which the capture devices keep their records in, skips a line it can not read, so that the rest of the
store still works.
