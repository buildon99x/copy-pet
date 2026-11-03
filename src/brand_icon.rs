//! User-selected C identity: a relaxed Ragdoll in a terracotta document pocket.
//! Runtime decodes one 256px derivative only while making the startup icon,
//! then drops it. Original artwork stays in art/, outside the app binary.
//! Offline exporters use draw_from with the full master, avoiding upscaling.

use tiny_skia::{Color, FillRule, FilterQuality, GradientStop, LinearGradient,
    Paint, Path, PathBuilder, Pixmap, PixmapPaint, Point, SpreadMode, Transform};

#[derive(Clone, Copy)]
pub enum Surface { Windows, Macos }

fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let k = r * 0.552_284_8;
    let mut p = PathBuilder::new();
    p.move_to(x+r, y); p.line_to(x+w-r, y);
    p.cubic_to(x+w-r+k, y, x+w, y+r-k, x+w, y+r);
    p.line_to(x+w, y+h-r);
    p.cubic_to(x+w, y+h-r+k, x+w-r+k, y+h, x+w-r, y+h);
    p.line_to(x+r, y+h);
    p.cubic_to(x+r-k, y+h, x, y+h-r+k, x, y+h-r);
    p.line_to(x, y+r);
    p.cubic_to(x, y+r-k, x+r-k, y, x+r, y); p.close();
    p.finish().unwrap()
}

pub fn draw(pm: &mut Pixmap, surface: Surface) {
    let source=Pixmap::decode_png(include_bytes!("../assets/brand/selected-c-256.png"))
        .expect("verified brand icon derivative");
    draw_from(pm,surface,&source);
}

/// Exporters pass the preserved master. Only fit/alpha and the macOS tile are
/// applied; the selected artwork is not regenerated or recolored.
pub fn draw_from(pm: &mut Pixmap, surface: Surface, source: &Pixmap) {
    pm.fill(Color::TRANSPARENT);
    let size=pm.width().min(pm.height()) as f32;
    let base=Transform::from_scale(size/100.0,size/100.0);
    let (inset,span)=match surface {
        Surface::Windows => (0.0,100.0),
        Surface::Macos => {
            for i in (0..6).rev() {
                let spread=i as f32 * 0.45;
                let mut shadow=Paint::default();shadow.set_color_rgba8(66,43,30,5);
                pm.fill_path(&rounded(6.5-spread,8.4-spread,87.0+2.0*spread,86.0+2.0*spread,20.0+spread),
                    &shadow,FillRule::Winding,base,None);
            }
            let tile=rounded(6.5,6.5,87.0,87.0,20.0);
            let paint=Paint {shader:LinearGradient::new(Point::from_xy(20.0,7.0),Point::from_xy(75.0,94.0),
                vec![GradientStop::new(0.0,Color::from_rgba8(255,251,241,255)),
                     GradientStop::new(1.0,Color::from_rgba8(238,222,201,255))],
                SpreadMode::Pad,Transform::identity()).unwrap(),..Paint::default()};
            pm.fill_path(&tile,&paint,FillRule::Winding,base,None);
            (12.0,76.0)
        }
    };
    let fit=size*(span/100.0)/source.width().max(source.height()) as f32;
    pm.draw_pixmap(0,0,source.as_ref(),&PixmapPaint {quality:FilterQuality::Bicubic,..PixmapPaint::default()},
        Transform::from_scale(fit,fit).post_translate(size*inset/100.0,size*inset/100.0),None);
}
