//! Generates MINT's pixel sprites from the illustrations `docs/images/assets/mint-1.png`
//! to `mint-6.png`: the white background connected to the border becomes transparent,
//! and each 10 × 10 block of source pixels becomes one sprite pixel.

use image::RgbImage;
use std::{env, fmt::Write as _, fs, path::PathBuf};

/// Source pixels per sprite pixel along each axis.
const CELL: u32 = 10;
/// Crops in source pixels as `[left, top, right, bottom]`: the bust for the sidebar card
/// and the full figure for the settings page. Both keep the effects drawn beside her.
const BUST: [u32; 4] = [160, 20, 1090, 800];
const FIGURE: [u32; 4] = [160, 20, 1090, 1240];
/// Pixels whose channels are all at least this bright count as background when they
/// connect to the image border.
const BACKGROUND: u8 = 246;
/// Share of the darkest pixels averaged per cell, which keeps outlines from fading.
const DARK_SHARE: f32 = 0.7;

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let assets = root.join("../../docs/images/assets");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=build.rs");
    let mut code = String::new();
    for (name, crop) in [("BUST", BUST), ("FIGURE", FIGURE)] {
        let size = [(crop[2] - crop[0]) / CELL, (crop[3] - crop[1]) / CELL];
        writeln!(
            code,
            "pub const {name}_SIZE: [usize; 2] = [{}, {}];",
            size[0], size[1]
        )
        .unwrap();
    }
    let mut lists = [String::new(), String::new()];
    for n in 1..=6 {
        let path = assets.join(format!("mint-{n}.png"));
        println!("cargo:rerun-if-changed={}", path.display());
        let image = image::open(&path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .to_rgb8();
        let mask = foreground(&image);
        for (list, (name, crop)) in lists.iter_mut().zip([("bust", BUST), ("figure", FIGURE)]) {
            let file = out.join(format!("{name}-{n}.rgba"));
            fs::write(&file, pixelize(&image, &mask, crop)).unwrap();
            write!(list, "include_bytes!({:?}), ", file.to_str().unwrap()).unwrap();
        }
    }
    writeln!(code, "pub const BUST: [&[u8]; 6] = [{}];", lists[0]).unwrap();
    writeln!(code, "pub const FIGURE: [&[u8]; 6] = [{}];", lists[1]).unwrap();
    fs::write(out.join("mascot_sprites.rs"), code).unwrap();
}

/// Marks every pixel except the bright background reachable from the border.
fn foreground(image: &RgbImage) -> Vec<bool> {
    let (w, h) = image.dimensions();
    let mut keep = vec![true; (w * h) as usize];
    let mut stack = Vec::new();
    let visit = |x: u32, y: u32, keep: &mut [bool], stack: &mut Vec<(u32, u32)>| {
        let i = (y * w + x) as usize;
        if keep[i] && image.get_pixel(x, y).0.iter().all(|&c| c >= BACKGROUND) {
            keep[i] = false;
            stack.push((x, y));
        }
    };
    for x in 0..w {
        visit(x, 0, &mut keep, &mut stack);
        visit(x, h - 1, &mut keep, &mut stack);
    }
    for y in 0..h {
        visit(0, y, &mut keep, &mut stack);
        visit(w - 1, y, &mut keep, &mut stack);
    }
    while let Some((x, y)) = stack.pop() {
        if x > 0 {
            visit(x - 1, y, &mut keep, &mut stack);
        }
        if x + 1 < w {
            visit(x + 1, y, &mut keep, &mut stack);
        }
        if y > 0 {
            visit(x, y - 1, &mut keep, &mut stack);
        }
        if y + 1 < h {
            visit(x, y + 1, &mut keep, &mut stack);
        }
    }
    keep
}

/// RGBA sprite of a crop: a cell is opaque when most of its pixels are foreground, and
/// takes the mean colour of its darker foreground pixels.
fn pixelize(image: &RgbImage, mask: &[bool], crop: [u32; 4]) -> Vec<u8> {
    let width = image.width();
    let (columns, rows) = ((crop[2] - crop[0]) / CELL, (crop[3] - crop[1]) / CELL);
    let mut sprite = Vec::with_capacity((columns * rows * 4) as usize);
    let mut cell = Vec::with_capacity((CELL * CELL) as usize);
    for row in 0..rows {
        for column in 0..columns {
            cell.clear();
            for y in crop[1] + row * CELL..crop[1] + (row + 1) * CELL {
                for x in crop[0] + column * CELL..crop[0] + (column + 1) * CELL {
                    if mask[(y * width + x) as usize] {
                        cell.push(image.get_pixel(x, y).0);
                    }
                }
            }
            if cell.len() * 2 < (CELL * CELL) as usize {
                sprite.extend([0; 4]);
                continue;
            }
            cell.sort_by_key(|p| p.iter().map(|&c| c as u32).sum::<u32>());
            let dark = &cell[..((cell.len() as f32 * DARK_SHARE).ceil() as usize).max(1)];
            for channel in 0..3 {
                let sum: u32 = dark.iter().map(|p| p[channel] as u32).sum();
                sprite.push(((sum as f32) / dark.len() as f32).round() as u8);
            }
            sprite.push(255);
        }
    }
    sprite
}
