//! Generates the deterministic source image for the Emu198x smoke fixtures.
//!
//! The content is chosen to exercise the converter, not to look pretty:
//! a horizontal hue ramp (out-of-gamut colours that force dither mixing),
//! a vertical luminance ramp, solid calibration bars (colours most machine
//! palettes can hit exactly), and a high-frequency checkerboard strip
//! (attribute-cell stress). Pure integer arithmetic — same bytes on every
//! platform, so the committed PNG never drifts.
//!
//! Usage: `cargo run --example gen_smoke_source -- <output.png>`

use image::{ImageBuffer, Rgb};

const W: u32 = 320;
const H: u32 = 200;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "smoke-source.png".to_string());

    let img = ImageBuffer::from_fn(W, H, |x, y| {
        if y < 80 {
            // Hue ramp x luminance ramp.
            let r = (x * 255 / (W - 1)) as u8;
            let g = (y * 255 / 79) as u8;
            let b = 255 - r;
            Rgb([r, g, b])
        } else if y < 120 {
            // Calibration bars: black, white, red, green, blue, cyan, magenta, yellow.
            const BARS: [[u8; 3]; 8] = [
                [0, 0, 0],
                [255, 255, 255],
                [255, 0, 0],
                [0, 255, 0],
                [0, 0, 255],
                [0, 255, 255],
                [255, 0, 255],
                [255, 255, 0],
            ];
            Rgb(BARS[(x * 8 / W) as usize])
        } else if y < 160 {
            // Mid-grey gradient (gamma-correctness witness).
            let v = (x * 255 / (W - 1)) as u8;
            Rgb([v, v, v])
        } else {
            // 2x2 checkerboard strip in orange/blue (cell-clash stress).
            if (x / 2 + y / 2) % 2 == 0 {
                Rgb([240, 120, 0])
            } else {
                Rgb([0, 60, 200])
            }
        }
    });

    img.save(&path).expect("failed to write smoke source PNG");
    println!("wrote {path}");
}
