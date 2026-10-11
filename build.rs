use png::{BitDepth, ColorType, Encoder};
use std::{env, fs::File, io::BufWriter, path::Path};

const WIDTH: usize = 384;
const HEIGHT: usize = 256;

fn main() {
    let output = Path::new(&env::var("OUT_DIR").expect("OUT_DIR is set")).join("default_world.png");
    let file = File::create(output).expect("create default world map");
    let mut encoder = Encoder::new(BufWriter::new(file), WIDTH as u32, HEIGHT as u32);
    encoder.set_color(ColorType::Rgb);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().expect("write PNG header");
    let mut pixels = Vec::with_capacity(WIDTH * HEIGHT * 3);

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let nx = x as f32 / WIDTH as f32;
            let ny = y as f32 / HEIGHT as f32;
            let wave = (nx * 19.0).sin() * 0.035 + (ny * 23.0).cos() * 0.025;
            let west = ((nx - 0.34) / 0.28).powi(2) + ((ny - 0.54) / 0.38).powi(2);
            let east = ((nx - 0.76) / 0.21).powi(2) + ((ny - 0.42) / 0.27).powi(2);
            let island = west < 1.0 + wave || east < 1.0 + wave;
            let color = if !island {
                if (x + y * 3) % 11 < 4 {
                    [29, 55, 91]
                } else {
                    [42, 84, 122]
                }
            } else {
                let edge = west.min(east);
                let ridge = ((nx * 41.0).sin() + (ny * 33.0).cos() + (nx * ny * 180.0).sin()) / 3.0;
                if edge > 0.88 {
                    [198, 174, 116]
                } else if (nx - 0.49).abs() < 0.006 && (ny - (0.74 - nx * 0.35)).abs() < 0.018 {
                    [188, 151, 93]
                } else if ridge > 0.51 && edge < 0.69 {
                    [119, 113, 105]
                } else if ridge > 0.12 && (x / 13 + y / 9) % 3 == 0 {
                    [38, 94, 68]
                } else {
                    [83, 143, 78]
                }
            };
            pixels.extend_from_slice(&color);
        }
    }

    writer
        .write_image_data(&pixels)
        .expect("write default world map");
}
