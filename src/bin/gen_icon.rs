//! Dev tool: regenerates assets/clipcat.ico from the vector cat art.
//! Run once with `cargo run --bin gen_icon`; the result is committed and
//! embedded into the exe by build.rs.

use tiny_skia::Pixmap;

fn demul(pm: &Pixmap) -> Vec<u8> {
    // tiny-skia stores premultiplied RGBA; icons want straight alpha
    pm.data()
        .chunks_exact(4)
        .flat_map(|px| {
            let a = px[3] as u32;
            if a == 0 {
                [0u8, 0, 0, 0]
            } else {
                [
                    ((px[0] as u32 * 255 + a / 2) / a).min(255) as u8,
                    ((px[1] as u32 * 255 + a / 2) / a).min(255) as u8,
                    ((px[2] as u32 * 255 + a / 2) / a).min(255) as u8,
                    px[3],
                ]
            }
        })
        .collect()
}

/// 32bpp BMP-format ICO entry: BITMAPINFOHEADER + bottom-up BGRA + AND mask.
fn bmp_entry(pm: &Pixmap) -> Vec<u8> {
    let s = pm.width() as usize;
    let rgba = demul(pm);
    let mut v = Vec::new();
    v.extend_from_slice(&40u32.to_le_bytes()); // header size
    v.extend_from_slice(&(s as i32).to_le_bytes());
    v.extend_from_slice(&((s * 2) as i32).to_le_bytes()); // height incl. mask
    v.extend_from_slice(&1u16.to_le_bytes()); // planes
    v.extend_from_slice(&32u16.to_le_bytes()); // bpp
    v.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    v.extend_from_slice(&((s * s * 4) as u32).to_le_bytes());
    v.extend_from_slice(&[0u8; 16]); // ppm x2, clr used/important
    for y in (0..s).rev() {
        for x in 0..s {
            let i = (y * s + x) * 4;
            v.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    // 1bpp AND mask, rows padded to 32 bits; all zero (alpha drives shape)
    let row_bytes = s.div_ceil(32) * 4;
    v.resize(v.len() + row_bytes * s, 0u8);
    v
}

fn main() {
    let sizes = [16u32, 32, 48, 256];
    let mut images = Vec::new();
    for &s in &sizes {
        let mut pm = Pixmap::new(s, s).unwrap();
        clipcat::render::draw_icon_scaled(&mut pm, s as f32 / 32.0);
        let png = s == 256;
        let data = if png {
            pm.encode_png().unwrap()
        } else {
            bmp_entry(&pm)
        };
        images.push((s, data));
    }

    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    ico.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (s, data) in &images {
        let dim = if *s >= 256 { 0u8 } else { *s as u8 };
        ico.push(dim);
        ico.push(dim);
        ico.push(0); // colors
        ico.push(0); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bpp
        ico.extend_from_slice(&(data.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in &images {
        ico.extend_from_slice(data);
    }

    std::fs::create_dir_all("assets").unwrap();
    std::fs::write("assets/clipcat.ico", &ico).unwrap();
    println!("wrote assets/clipcat.ico ({} bytes)", ico.len());
}
