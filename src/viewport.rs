//! The core keeps layout in logical units. Only the portable macOS boundary
//! uses backing density; screen positions and captured cursor deltas stay physical.
use crate::panel::Rect;

#[derive(Clone, Copy, Debug)]
pub struct Density(f64);

impl Density {
    pub fn new(scale: f64) -> Self {
        Self(if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        })
    }
    pub fn scale(self) -> f32 {
        self.0 as f32
    }
    pub fn physical_size(self, logical: (i32, i32)) -> (u32, u32) {
        (
            (logical.0 as f64 * self.0).round().max(1.0) as u32,
            (logical.1 as f64 * self.0).round().max(1.0) as u32,
        )
    }
    pub fn logical(self, physical: (f64, f64)) -> (f32, f32) {
        ((physical.0 / self.0) as f32, (physical.1 / self.0) as f32)
    }
    pub fn physical_shift(self, logical: (i32, i32)) -> (i32, i32) {
        (
            (logical.0 as f64 * self.0).round() as i32,
            (logical.1 as f64 * self.0).round() as i32,
        )
    }
    pub fn screen_rect(self, origin: (f32, f32), logical: Rect) -> Rect {
        let s = self.scale();
        Rect {
            x: origin.0 + logical.x * s,
            y: origin.1 + logical.y * s,
            w: logical.w * s,
            h: logical.h * s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retina_size_pointer_drag_and_monitor_transition() {
        let retina = Density::new(2.0);
        assert_eq!(retina.physical_size((352, 362)), (704, 724));
        assert_eq!(retina.logical((240.0, 360.0)), (120.0, 180.0));
        assert_eq!(retina.logical((14.0, -10.0)), (7.0, -5.0));
        assert_eq!(retina.physical_shift((7, -5)), (14, -10));
        assert_eq!(Density::new(1.0).physical_size((352, 362)), (352, 362));
        assert_eq!(Density::new(1.5).physical_size((352, 362)), (528, 543));
    }
    #[test]
    fn fit_operates_in_screen_pixels_then_returns_logical_shift() {
        let d = Density::new(2.0);
        let card = d.screen_rect(
            (800.0, 20.0),
            Rect {
                x: 10.0,
                y: 5.0,
                w: 352.0,
                h: 362.0,
            },
        );
        let (dx, dy) = crate::panel::fit_delta(
            card,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1400.0,
                h: 900.0,
            },
        );
        assert_eq!(d.logical((dx as f64, dy as f64)), (-62.0, 0.0));
    }
}
