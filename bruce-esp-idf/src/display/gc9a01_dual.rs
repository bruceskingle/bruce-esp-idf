//! The two GC9A01 round LCDs of the Waveshare ESP32-S3-DualEye-LCD-1.28, each 240x240, 16 bit colour, on one SPI bus,
//! SPI2, sharing its clock, data and DC lines, each with its own chip select, reset and backlight. They are a pair of
//! eyes, so each is drawn on separately, in a framebuffer of its own in PSRAM, which flush sends to its panel when it
//! has changed, see spawn_flusher: only the rows which changed, a strip at a time through two small buffers in internal
//! RAM which DMA can read, used in turn, so that one is filled while the other is sent.
//!
//! The left screen as the board is looked at, LCD1, or LCD2 with the board turned half way round, see
//! Gc9a01Dual::set_orientation, is the platform's display, which the status, adoption's pairing code and features such
//! as the analog clock draw on, see Gc9a01DualDisplayManager::display; the right is the project's, see right and eyes.
//!
//! The panels are driven through ESP-IDF's esp_lcd SPI panel IO, two on the one bus, as Waveshare's demo,
//! examples/esp-idf/LCD_03_Animated_Eye1, does, at its 80 MHz, with its settings: colours inverted, BGR, rows and
//! columns swapped on both, and the right one mirrored both ways, but each flipped top to bottom, see LEFT_MADCTL and
//! RIGHT_MADCTL. The start-up sequence is the GC9A01's usual one, as mipidsi's and Espressif's esp_lcd_gc9a01 drivers
//! send it.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bruce_embedded_std::graphics::{Color, Console, DisplayManager, wrap};
use bruce_embedded_std::{InitStatus, Status};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use esp_idf_sys::{self as sys, esp};

/// Each panel's size, square, though only the circle inside it can be seen.
pub const SIZE: u32 = 240;
const SIDE: usize = SIZE as usize;
/// The square inside the circle, where text can be read in full, see Gc9a01DualDisplayManager::show_status: its
/// side, a little less than the circle's diameter over the square root of two, and its distance from the edge.
const INSIDE_SIDE: u32 = 166;
const INSIDE_OFFSET: i32 = ((SIZE - INSIDE_SIDE) / 2) as i32;

/// The rows sent at a time, through each of two buffers in internal RAM, 9.6 KB each.
const STRIP_ROWS: usize = 20;
const STRIP_BYTES: usize = STRIP_ROWS * SIDE * 2;
/// The SPI clock, which Waveshare's demo uses. A whole panel, 115 KB, takes about 12 ms.
const PIXEL_CLOCK_HZ: u32 = 80_000_000;
/// How often the flusher looks for changes to send, see spawn_flusher, 25 times a second, so that an animation drawn
/// with no flush of its own still moves; one which wants every frame seen at once calls flush itself.
const FLUSH_INTERVAL: Duration = Duration::from_millis(40);

/// The backlights' PWM, LEDC's low speed timer 0, at 5 kHz, with 13 bits of duty, one channel each, see
/// Gc9a01::set_brightness.
const BACKLIGHT_TIMER: sys::ledc_timer_t = sys::ledc_timer_t_LEDC_TIMER_0;
const BACKLIGHT_MODE: sys::ledc_mode_t = sys::ledc_mode_t_LEDC_LOW_SPEED_MODE;
const BACKLIGHT_FREQUENCY_HZ: u32 = 5_000;
const BACKLIGHT_FULL_DUTY: u32 = (1 << 13) - 1;

/// The memory access control, MADCTL, bits: rows and columns swapped, (MV), columns mirrored (MX), rows mirrored (MY),
/// and BGR order.
const MADCTL_MY: u8 = 0x80;
const MADCTL_MX: u8 = 0x40;
const MADCTL_MV: u8 = 0x20;
const MADCTL_BGR: u8 = 0x08;
/// Each panel's MADCTL: Waveshare's demo's, for its default rotation, see the module comment, with MX toggled, as the
/// demo's left screen was mirrored top to bottom on the board, which its eyes, the same either way up, do not show.
/// Found on the board, 25 Nov 2026: with rows and columns swapped, MX flips these panels top to bottom and MY left to
/// right. The right one is flipped as the left, as the panels are the same, mounted half a turn apart.
const LEFT_MADCTL: u8 = MADCTL_MX | MADCTL_MV | MADCTL_BGR;
const RIGHT_MADCTL: u8 = MADCTL_MY | MADCTL_MV | MADCTL_BGR;

/// The GC9A01's start-up, each command and its parameters, between the reset and MADCTL, see the module comment.
const INIT: &[(u8, &[u8])] = &[
    (0xEF, &[]), // Inter register enable 2
    (0xEB, &[0x14]),
    (0xFE, &[]), // Inter register enable 1
    (0xEF, &[]),
    (0xEB, &[0x14]),
    (0x84, &[0x40]),
    (0x85, &[0xFF]),
    (0x86, &[0xFF]),
    (0x87, &[0xFF]),
    (0x88, &[0x0A]),
    (0x89, &[0x21]),
    (0x8A, &[0x00]),
    (0x8B, &[0x80]),
    (0x8C, &[0x01]),
    (0x8D, &[0x01]),
    (0x8E, &[0xFF]),
    (0x8F, &[0xFF]),
    (0xB6, &[0x00, 0x20]), // Display function control
];

/// The rest of the start-up, after MADCTL and the pixel format.
const INIT_AFTER_MADCTL: &[(u8, &[u8])] = &[
    (0x90, &[0x08, 0x08, 0x08, 0x08]),
    (0xBD, &[0x06]),
    (0xBC, &[0x00]),
    (0xFF, &[0x60, 0x01, 0x04]),
    (0xC3, &[0x13]), // Power control 2
    (0xC4, &[0x13]), // Power control 3
    (0xC9, &[0x22]), // Power control 4
    (0xBE, &[0x11]),
    (0xE1, &[0x10, 0x0E]),
    (0xDF, &[0x20, 0x0C, 0x02]),
    (0xF0, &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A]), // Gamma 1
    (0xF1, &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F]), // Gamma 2
    (0xF2, &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A]), // Gamma 3
    (0xF3, &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F]), // Gamma 4
    (0xED, &[0x18, 0x0B]),
    (0xAE, &[0x77]),
    (0xCD, &[0x63]),
    (0x70, &[0x07, 0x07, 0x04, 0x0E, 0x0F, 0x09, 0x07, 0x08, 0x03]),
    (0xE8, &[0x34]), // Frame rate
    (
        0x62,
        &[0x18, 0x0D, 0x71, 0xED, 0x70, 0x70, 0x18, 0x0F, 0x71, 0xEF, 0x70, 0x70],
    ),
    (
        0x63,
        &[0x18, 0x11, 0x71, 0xF1, 0x70, 0x70, 0x18, 0x13, 0x71, 0xF3, 0x70, 0x70],
    ),
    (0x64, &[0x28, 0x29, 0xF1, 0x01, 0xF1, 0x00, 0x07]),
    (0x66, &[0x3C, 0x00, 0xCD, 0x67, 0x45, 0x45, 0x10, 0x00, 0x00, 0x00]),
    (0x67, &[0x00, 0x3C, 0x00, 0x00, 0x00, 0x01, 0x54, 0x10, 0x32, 0x98]),
    (0x74, &[0x10, 0x85, 0x80, 0x00, 0x00, 0x4E, 0x00]),
    (0x98, &[0x3E, 0x07]),
    (0x21, &[]), // Colours inverted, as the panels need
];

/// The pins of the panels, by number, as esp_lcd takes them: the bus's, shared, and each panel's own.
pub struct Pins {
    pub clock: i32,
    pub mosi: i32,
    pub dc: i32,
    pub left: PanelPins,
    pub right: PanelPins,
}

/// A panel's own pins.
pub struct PanelPins {
    pub chip_select: i32,
    pub reset: i32,
    pub backlight: i32,
}

/// One of the panels, as a DrawTarget of Rgb565, 240x240.
pub struct Gc9a01 {
    io: sys::esp_lcd_panel_io_handle_t,
    /// A pixel each, row by row.
    frame: Vec<Rgb565>,
    /// The first and last rows which have changed since the frame was last sent, if any, see flush.
    dirty: Option<(usize, usize)>,
    /// The backlight's PWM channel, see set_brightness.
    backlight: sys::ledc_channel_t,
    /// The panel's memory access control, see turn_half_way.
    madctl: u8,
}

// The panel IO is only used by whoever holds the display, behind the display manager's mutex.
unsafe impl Send for Gc9a01 {}

impl Gc9a01 {
    /// Adds the panel to the bus, resets and starts it, with madctl, and clears it to black, with its backlight off, on
    /// the PWM channel backlight, of BACKLIGHT_TIMER, which must be set up.
    fn new(pins: &PanelPins, dc: i32, madctl: u8, backlight: sys::ledc_channel_t) -> anyhow::Result<Self> {
        let mut config: sys::esp_lcd_panel_io_spi_config_t = Default::default();
        config.cs_gpio_num = pins.chip_select;
        config.dc_gpio_num = dc;
        config.spi_mode = 0;
        config.pclk_hz = PIXEL_CLOCK_HZ;
        // One in flight while the other strip is filled, see Strips.
        config.trans_queue_depth = 1;
        config.lcd_cmd_bits = 8;
        config.lcd_param_bits = 8;
        let mut io: sys::esp_lcd_panel_io_handle_t = std::ptr::null_mut();
        esp!(unsafe {
            sys::esp_lcd_new_panel_io_spi(
                sys::spi_host_device_t_SPI2_HOST as sys::esp_lcd_spi_bus_handle_t,
                &config,
                &mut io,
            )
        })?;

        // The backlight driven off, as it may not be off if left floating, until set_backlight, once there is a
        // picture, so that what the panel had in its memory is not seen.
        let channel = sys::ledc_channel_config_t {
            gpio_num: pins.backlight,
            speed_mode: BACKLIGHT_MODE,
            channel: backlight,
            timer_sel: BACKLIGHT_TIMER,
            duty: 0,
            ..Default::default()
        };
        esp!(unsafe { sys::ledc_channel_config(&channel) })?;
        // A hardware reset, held low for 10 ms, then 120 ms for the panel to come out of it.
        output(pins.reset, false)?;
        std::thread::sleep(Duration::from_millis(10));
        esp!(unsafe { sys::gpio_set_level(pins.reset, 1) })?;
        std::thread::sleep(Duration::from_millis(120));

        let mut panel = Gc9a01 {
            io,
            frame: vec![Rgb565::BLACK; SIDE * SIDE],
            dirty: Some((0, SIDE - 1)),
            backlight,
            madctl,
        };
        for (command, parameters) in INIT {
            panel.command(*command, parameters)?;
        }
        panel.command(0x36, &[madctl])?; // Memory access control
        panel.command(0x3A, &[0x55])?; // 16 bit colour
        for (command, parameters) in INIT_AFTER_MADCTL {
            panel.command(*command, parameters)?;
        }
        panel.command(0x11, &[])?; // Sleep out
        std::thread::sleep(Duration::from_millis(120));
        panel.command(0x29, &[])?; // Display on
        std::thread::sleep(Duration::from_millis(20));
        Ok(panel)
    }

    /// Turns what the panel shows half way round, by mirroring it both ways, MX and MY, and sends it all again.
    fn turn_half_way(&mut self) -> anyhow::Result<()> {
        self.madctl ^= MADCTL_MX | MADCTL_MY;
        self.command(0x36, &[self.madctl])?;
        self.dirty = Some((0, SIDE - 1));
        Ok(())
    }

    /// Turns the backlight on, at full brightness, or off.
    pub fn set_backlight(&mut self, on: bool) -> anyhow::Result<()> {
        self.set_brightness(if on { 100 } else { 0 })
    }

    /// Sets the backlight's brightness, in percent, from 0, off, to 100, full. The light given is the square of it, as
    /// brightness is seen, roughly, so that e.g. 10%, for the night, gives a hundredth of the light, faint but seen in
    /// the dark.
    pub fn set_brightness(&mut self, percent: u8) -> anyhow::Result<()> {
        let fraction = percent.min(100) as u32;
        let duty = (BACKLIGHT_FULL_DUTY * fraction * fraction + 5_000) / 10_000;
        esp!(unsafe { sys::ledc_set_duty(BACKLIGHT_MODE, self.backlight, duty) })?;
        esp!(unsafe { sys::ledc_update_duty(BACKLIGHT_MODE, self.backlight) })?;
        Ok(())
    }

    /// Sends a command and its parameters, after waiting for the panel's pixels in flight to be sent, as esp_lcd's
    /// tx_param does.
    fn command(&mut self, command: u8, parameters: &[u8]) -> anyhow::Result<()> {
        let (pointer, length) = if parameters.is_empty() {
            (std::ptr::null(), 0)
        } else {
            (parameters.as_ptr() as *const _, parameters.len())
        };
        esp!(unsafe { sys::esp_lcd_panel_io_tx_param(self.io, command as i32, pointer, length) })?;
        Ok(())
    }

    /// Fills strip with the rows from first, at most STRIP_ROWS of them, big endian, returning how many bytes that is.
    fn fill_strip(&self, strip: *mut u8, first: usize, rows: usize) -> usize {
        let length = rows * SIDE * 2;
        let strip = unsafe { std::slice::from_raw_parts_mut(strip, length) };
        for (index, pixel) in self.frame[first * SIDE..(first + rows) * SIDE].iter().enumerate() {
            let bytes = pixel.into_storage().to_be_bytes();
            strip[index * 2] = bytes[0];
            strip[index * 2 + 1] = bytes[1];
        }
        length
    }

    /// Sends the rows which have changed to the panel, if any, a strip at a time through strips, see the module
    /// comment, and waits for the last to be sent, so that the strips are free for the other panel.
    fn flush(&mut self, strips: &Strips) -> anyhow::Result<()> {
        let Some((first, last)) = self.dirty.take() else {
            return Ok(());
        };
        let end = SIDE as u16 - 1;
        self.command(0x2A, &[0, 0, (end >> 8) as u8, end as u8])?; // Columns, all of them
        self.command(0x2B, &[(first >> 8) as u8, first as u8, (last >> 8) as u8, last as u8])?; // Rows, those which changed
        for (index, row) in (first..=last).step_by(STRIP_ROWS).enumerate() {
            let strip = strips.0[index % 2];
            // The queue is one deep, so this strip's last send, two ago, has finished once the last one was queued.
            let length = self.fill_strip(strip, row, STRIP_ROWS.min(last + 1 - row));
            // Memory write, from the first row, then memory write continue.
            let memory_write = if index == 0 { 0x2C } else { 0x3C };
            esp!(unsafe { sys::esp_lcd_panel_io_tx_color(self.io, memory_write, strip as *const _, length) })?;
        }
        // A NOP, which waits for the last strip to be sent.
        self.command(0x00, &[])?;
        Ok(())
    }
}

impl OriginDimensions for Gc9a01 {
    fn size(&self) -> Size {
        Size::new(SIZE, SIZE)
    }
}

impl DrawTarget for Gc9a01 {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x >= 0 && point.y >= 0 && (point.x as u32) < SIZE && (point.y as u32) < SIZE {
                let y = point.y as usize;
                self.frame[y * SIDE + point.x as usize] = color;
                self.dirty = Some(match self.dirty {
                    Some((first, last)) => (first.min(y), last.max(y)),
                    None => (y, y),
                });
            }
        }
        Ok(())
    }

    /// Whole rows, e.g. an animation's frame, or the rows of it which changed, are copied a row at a time, and only the
    /// rows which differ are marked to be sent, so that only they are, see flush. Any other area is drawn a pixel at a
    /// time.
    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let (top, height) = (area.top_left.y, area.size.height as i32);
        if area.top_left.x != 0 || area.size.width != SIZE || top < 0 || top + height > SIZE as i32 {
            return self.draw_iter(area.points().zip(colors).map(|(point, color)| Pixel(point, color)));
        }
        let mut colors = colors.into_iter();
        let mut row = [Rgb565::BLACK; SIDE];
        for y in top as usize..(top + height) as usize {
            for pixel in row.iter_mut() {
                *pixel = colors.next().unwrap_or(Rgb565::BLACK);
            }
            let frame_row = &mut self.frame[y * SIDE..(y + 1) * SIDE];
            if *frame_row != row {
                frame_row.copy_from_slice(&row);
                self.dirty = Some(match self.dirty {
                    Some((first, last)) => (first.min(y), last.max(y)),
                    None => (y, y),
                });
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let area = area.intersection(&self.bounding_box());
        let Some(bottom_right) = area.bottom_right() else {
            return Ok(());
        };
        let (left, right) = (area.top_left.x as usize, bottom_right.x as usize);
        let (top, bottom) = (area.top_left.y as usize, bottom_right.y as usize);
        for y in top..=bottom {
            self.frame[y * SIDE + left..=y * SIDE + right].fill(color);
        }
        self.dirty = Some(match self.dirty {
            Some((first, last)) => (first.min(top), last.max(bottom)),
            None => (top, bottom),
        });
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.frame.fill(color);
        self.dirty = Some((0, SIDE - 1));
        Ok(())
    }
}

/// The two strips, see STRIP_ROWS, in internal RAM which DMA can read, which both panels use, one after the other.
struct Strips([*mut u8; 2]);

// Only used by whoever holds the display, behind the display manager's mutex.
unsafe impl Send for Strips {}

/// Both panels, see the module comment.
pub struct Gc9a01Dual {
    /// The panel on the left as the board is looked at, LCD1, or LCD2 with the board turned half way round, see
    /// set_orientation, and the one on the right.
    left: Gc9a01,
    right: Gc9a01,
    strips: Strips,
    /// Whether the board is turned half way round, see set_orientation.
    turned: bool,
}

impl Gc9a01Dual {
    /// Starts the SPI bus on SPI2 and both panels, and clears them to black, with their backlights off, which the
    /// platform turns on, unless the Screen feature's setting says to keep them off, see features/screen.rs.
    pub fn new(pins: Pins) -> anyhow::Result<Self> {
        let mut bus: sys::spi_bus_config_t = Default::default();
        bus.sclk_io_num = pins.clock;
        bus.__bindgen_anon_1.mosi_io_num = pins.mosi;
        // The panels' data out, on GPIO40, is not read.
        bus.__bindgen_anon_2.miso_io_num = -1;
        bus.__bindgen_anon_3.quadwp_io_num = -1;
        bus.__bindgen_anon_4.quadhd_io_num = -1;
        bus.data4_io_num = -1;
        bus.data5_io_num = -1;
        bus.data6_io_num = -1;
        bus.data7_io_num = -1;
        bus.max_transfer_sz = STRIP_BYTES as i32;
        esp!(unsafe {
            sys::spi_bus_initialize(
                sys::spi_host_device_t_SPI2_HOST,
                &bus,
                sys::spi_common_dma_t_SPI_DMA_CH_AUTO,
            )
        })?;

        let strip = || -> anyhow::Result<*mut u8> {
            let strip = unsafe { sys::heap_caps_malloc(STRIP_BYTES, sys::MALLOC_CAP_DMA | sys::MALLOC_CAP_INTERNAL) }
                as *mut u8;
            anyhow::ensure!(!strip.is_null(), "no internal RAM for the displays' strips");
            Ok(strip)
        };
        let timer = sys::ledc_timer_config_t {
            speed_mode: BACKLIGHT_MODE,
            duty_resolution: sys::ledc_timer_bit_t_LEDC_TIMER_13_BIT,
            timer_num: BACKLIGHT_TIMER,
            freq_hz: BACKLIGHT_FREQUENCY_HZ,
            clk_cfg: sys::soc_periph_ledc_clk_src_legacy_t_LEDC_AUTO_CLK,
            ..Default::default()
        };
        esp!(unsafe { sys::ledc_timer_config(&timer) })?;
        let mut displays = Gc9a01Dual {
            left: Gc9a01::new(&pins.left, pins.dc, LEFT_MADCTL, sys::ledc_channel_t_LEDC_CHANNEL_0)?,
            right: Gc9a01::new(&pins.right, pins.dc, RIGHT_MADCTL, sys::ledc_channel_t_LEDC_CHANNEL_1)?,
            strips: Strips([strip()?, strip()?]),
            turned: false,
        };
        displays.flush()?;
        Ok(displays)
    }

    /// Which way up the board is: Rotate0, with its USB port at the bottom, as it comes, or Rotate180, turned half way
    /// round, e.g. so that the cable comes out of the top, for which each panel's picture is turned half way round, and
    /// they swap sides, so that the left is still the one on the left. A pair of eyes cannot be turned a quarter of the
    /// way round, so Rotate90 and Rotate270 are refused.
    pub fn set_orientation(&mut self, orientation: bruce_embedded_std::DisplayOrientation) -> anyhow::Result<()> {
        use bruce_embedded_std::DisplayOrientation;
        let turned = match orientation {
            DisplayOrientation::Rotate0 => false,
            DisplayOrientation::Rotate180 => true,
            _ => anyhow::bail!("The DualEye's screens can only be turned half way round, not {orientation:?}"),
        };
        if turned != self.turned {
            self.turned = turned;
            std::mem::swap(&mut self.left, &mut self.right);
            self.left.turn_half_way()?;
            self.right.turn_half_way()?;
        }
        Ok(())
    }

    /// Turns both backlights on or off, e.g. off for a board run headless, see features/screen.rs.
    pub fn set_backlight(&mut self, on: bool) -> anyhow::Result<()> {
        self.left.set_backlight(on)?;
        self.right.set_backlight(on)
    }

    /// Sets both backlights' brightness, in percent, see Gc9a01::set_brightness.
    pub fn set_brightness(&mut self, percent: u8) -> anyhow::Result<()> {
        self.left.set_brightness(percent)?;
        self.right.set_brightness(percent)
    }

    /// Sends what has changed on either panel to it, see Gc9a01::flush.
    pub fn flush(&mut self) -> anyhow::Result<()> {
        self.left.flush(&self.strips)?;
        self.right.flush(&self.strips)
    }
}

/// Configures pin as an output, at level.
fn output(pin: i32, level: bool) -> anyhow::Result<()> {
    let config = sys::gpio_config_t {
        pin_bit_mask: 1u64 << pin,
        mode: sys::gpio_mode_t_GPIO_MODE_OUTPUT,
        ..Default::default()
    };
    esp!(unsafe { sys::gpio_config(&config) })?;
    esp!(unsafe { sys::gpio_set_level(pin, level as u32) })?;
    Ok(())
}

/// The pair of screens, whose left one is the DisplayManager's display, see the module comment.
pub struct Gc9a01DualDisplayManager {
    pub displays: Gc9a01Dual,
    /// See DisplayManager::overlay_active and generation.
    pub overlay: bool,
    pub generation: u32,
}

impl Gc9a01DualDisplayManager {
    /// The right screen, LCD2, which the platform does not draw on, other than to clear it with the left, see clear.
    pub fn right(&mut self) -> &mut Gc9a01 {
        &mut self.displays.right
    }

    /// Both screens, left and right, e.g. to draw a pair of eyes.
    pub fn eyes(&mut self) -> (&mut Gc9a01, &mut Gc9a01) {
        (&mut self.displays.left, &mut self.displays.right)
    }

    /// Sets both screens' brightness, in percent, e.g. dim at night, see Gc9a01::set_brightness.
    pub fn set_brightness(&mut self, percent: u8) -> anyhow::Result<()> {
        self.displays.set_brightness(percent)
    }

    /// Sends what has been drawn to the screens now, rather than at the flusher's next look, see spawn_flusher, e.g.
    /// at the end of each frame of an animation.
    pub fn flush(&mut self) -> anyhow::Result<()> {
        self.displays.flush()
    }
}

impl DisplayManager for Gc9a01DualDisplayManager {
    type Display = Gc9a01;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.displays.left
    }

    /// FONT_8X13, which is 20 characters across the square inside the circle, and 12 lines down, see show_status.
    fn console(&self) -> Console {
        Console {
            font: &embedded_graphics::mono_font::ascii::FONT_8X13,
            margin: 0,
        }
    }

    /// Shows the status as DisplayManager's own show_status does, but in the square inside the left screen's circle,
    /// as the corners of a round screen cannot be seen, and with the right screen black.
    fn show_status(&mut self, status: &Status, details: &[String]) -> anyhow::Result<()> {
        let (title, color) = match status {
            Status::Initializing(InitStatus::Starting) => ("Starting", Color::White),
            Status::Initializing(InitStatus::AwaitingClientIpAddress) => ("Joining the Wi-Fi", Color::Magenta),
            Status::Initializing(InitStatus::AwaitingTimeSync) => ("Getting the time", Color::Cyan),
            Status::Initializing(InitStatus::StartingFeatures) => ("Starting the features", Color::Green),
            Status::Running => return self.clear(),
            Status::Setup => ("Setup", Color::Yellow),
            Status::Error => ("Error", Color::Red),
        };
        self.fill_color(Color::Black)?;
        let console = self.console();
        let title_style = MonoTextStyle::new(console.font, self.map_color(&color));
        let detail_style = MonoTextStyle::new(console.font, self.map_color(&Color::White));
        let area = Rectangle::new(
            Point::new(INSIDE_OFFSET, INSIDE_OFFSET),
            Size::new(INSIDE_SIDE, INSIDE_SIDE),
        );
        let (across, down) = console.fit(&area);
        let mut inside = self.displays.left.cropped(&area);
        let lines = wrap(title, across).into_iter().map(|line| (line, title_style)).chain(
            details
                .iter()
                .flat_map(|detail| wrap(detail, across))
                .map(|line| (line, detail_style)),
        );
        for (index, (line, style)) in lines.take(down).enumerate() {
            let position = Point::new(0, (index as u32 * console.line_height()) as i32);
            let _ = Text::with_baseline(&line, position, style, Baseline::Top).draw(&mut inside);
        }
        Ok(())
    }

    fn overlay_active(&self) -> bool {
        self.overlay
    }

    fn generation(&self) -> u32 {
        self.generation
    }

    fn set_overlay(&mut self, active: bool) -> anyhow::Result<()> {
        if self.overlay && !active {
            self.clear()?;
        }
        self.overlay = active;
        Ok(())
    }

    fn clear(&mut self) -> anyhow::Result<()> {
        self.fill_color(Color::Black)?;
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    /// Fills both screens, so that the right one is cleared with the left, e.g. for adoption's pairing code.
    fn fill_color(&mut self, color: Color) -> anyhow::Result<()> {
        let color = self.map_color(&color);
        let _ = self.displays.left.clear(color);
        let _ = self.displays.right.clear(color);
        Ok(())
    }

    fn map_color(&self, color: &Color) -> Rgb565 {
        bruce_embedded_std::graphics::to_rgb565(color)
    }

    fn map_rgb8(&self, color: &rgb::RGB8) -> Rgb565 {
        bruce_embedded_std::graphics::rgb565_from_rgb8(color)
    }
}

/// Sends what has changed to the screens, every FLUSH_INTERVAL, for as long as the device runs.
pub fn spawn_flusher(display_manager: &Arc<Mutex<Gc9a01DualDisplayManager>>) -> anyhow::Result<()> {
    let display_manager = display_manager.clone();
    std::thread::Builder::new()
        .name("gc9a01 flush".into())
        .stack_size(4096)
        .spawn(move || {
            loop {
                std::thread::sleep(FLUSH_INTERVAL);
                if let Err(error) = display_manager.lock().unwrap().flush() {
                    log::warn!("Unable to update the displays, {error:#}");
                }
            }
        })?;
    Ok(())
}
