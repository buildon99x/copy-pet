//! Export the code-owned Windows ICO, macOS iconset/ICNS, masters and review.
//! Run `cargo run --release --bin gen_icon [-- OUTPUT_ROOT]` after art changes.

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
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "assets".into()));
    let windows = root.join("brand/windows");
    let iconset = root.join("brand/ClipCat.iconset");
    std::fs::create_dir_all(&windows).unwrap();
    std::fs::create_dir_all(&iconset).unwrap();
    let source = Pixmap::load_png("art/originals/icon-brand-2026-11-04/selected-c.png").unwrap();
    let mut runtime=Pixmap::new(256,256).unwrap();
    clipcat::brand_icon::draw_from(&mut runtime,clipcat::brand_icon::Surface::Windows,&source);
    runtime.save_png(root.join("brand/selected-c-256.png")).unwrap();
    let sizes = [16u32, 20, 24, 32, 40, 48, 64, 128, 256];
    let mut images = Vec::new();
    for &s in &sizes {
        let mut pm = Pixmap::new(s, s).unwrap();
        clipcat::brand_icon::draw_from(&mut pm,clipcat::brand_icon::Surface::Windows,&source);
        pm.save_png(windows.join(format!("icon-{s}.png"))).unwrap();
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

    std::fs::write(root.join("clipcat.ico"), &ico).unwrap();
    for s in [16u32, 32, 128, 256, 512] {
        for density in [1u32, 2] {
            let pixels=s*density;
            let mut image=Pixmap::new(pixels,pixels).unwrap();
            clipcat::brand_icon::draw_from(&mut image,clipcat::brand_icon::Surface::Macos,&source);
            let suffix=if density==2 {"@2x"} else {""};
            image.save_png(iconset.join(format!("icon_{s}x{s}{suffix}.png"))).unwrap();
        }
    }
    for (name,surface) in [("windows",clipcat::brand_icon::Surface::Windows),("macos",clipcat::brand_icon::Surface::Macos)] {
        let mut image=Pixmap::new(1024,1024).unwrap();
        clipcat::brand_icon::draw_from(&mut image,surface,&source);
        image.save_png(root.join(format!("brand/{name}-1024.png"))).unwrap();
    }
    review(&root,&source);
    #[cfg(target_os="macos")]
    assert!(std::process::Command::new("/usr/bin/iconutil").args(["-c","icns"])
        .arg(&iconset).arg("-o").arg(root.join("clipcat.icns")).status().unwrap().success());
    println!("wrote ICO, iconset, masters and review under {}",root.display());
}

fn review(root: &std::path::Path,source: &Pixmap) {
    use clipcat::brand_icon::{draw_from,Surface};
    use tiny_skia::{Color,PixmapPaint,Transform};
    let mut sheet=Pixmap::new(1400,900).unwrap();
    sheet.fill(Color::from_rgba8(242,239,233,255));
    let mut label=|text:&str,x:f32,y:f32,size:f32| {
        clipcat::sysfont::draw(&mut sheet,text,x,y,size,(49,63,79,255),Transform::identity());
    };
    label("ClipCat",48.0,30.0,3.0);
    label("A quiet companion for everything you copy.",48.0,91.0,1.25);
    label("WINDOWS",48.0,151.0,1.4);
    label("macOS",748.0,151.0,1.4);
    for (column,surface) in [(0,Surface::Windows),(1,Surface::Macos)] {
        let x=column*700;
        let mut hero=Pixmap::new(276,276).unwrap(); draw_from(&mut hero,surface,source);
        sheet.draw_pixmap(x+48,202,hero.as_ref(),&PixmapPaint::default(),Transform::identity(),None);
        for (row,bg) in [(0,(255,255,255)),(1,(28,36,48))] {
            let mut band=Pixmap::new(604,138).unwrap();
            band.fill(Color::from_rgba8(bg.0,bg.1,bg.2,255));
            for (i,size) in [16u32,24,32,48,64,96].iter().enumerate() {
                let mut icon=Pixmap::new(*size,*size).unwrap(); draw_from(&mut icon,surface,source);
                band.draw_pixmap(20+i as i32*94,14+(96-*size) as i32/2,icon.as_ref(),&PixmapPaint::default(),Transform::identity(),None);
                let ink=if row==0 {(49,63,79,255)} else {(220,230,242,255)};
                clipcat::sysfont::draw(&mut band,&size.to_string(),20.0+i as f32*94.0,112.0,0.85,ink,Transform::identity());
            }
            sheet.draw_pixmap(x+48,520+row*156,band.as_ref(),&PixmapPaint::default(),Transform::identity(),None);
        }
    }
    clipcat::sysfont::draw(&mut sheet,"Selected C / warm Ragdoll / terracotta document pocket",48.0,846.0,1.1,(81,92,105,255),Transform::identity());
    sheet.save_png(root.join("brand/icon-review.png")).unwrap();
}
