//! Stable identities, authored copy timelines and mouth tracks in pet canvas coordinates.
use crate::i18n::{self, Lang};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CatKind {
    #[default]
    Ragdoll,
    Birman,
    Siamese,
    Bombay,
    Ragamuffin,
    Persian,
    TurkishAngora,
    AmericanCurl,
    Munchkin,
    BritishShorthair,
    RussianBlue,
    Tuxedo,
}
impl CatKind {
    pub const ALL: [Self; 12] = [
        Self::Ragdoll,
        Self::Birman,
        Self::Siamese,
        Self::Bombay,
        Self::Ragamuffin,
        Self::Persian,
        Self::TurkishAngora,
        Self::AmericanCurl,
        Self::Munchkin,
        Self::BritishShorthair,
        Self::RussianBlue,
        Self::Tuxedo,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Ragdoll => "ragdoll",
            Self::Birman => "birman",
            Self::Siamese => "siamese",
            Self::Bombay => "bombay",
            Self::Ragamuffin => "ragamuffin",
            Self::Persian => "persian",
            Self::TurkishAngora => "turkish-angora",
            Self::AmericanCurl => "american-curl",
            Self::Munchkin => "munchkin",
            Self::BritishShorthair => "british-shorthair",
            Self::RussianBlue => "russian-blue",
            Self::Tuxedo => "tuxedo",
        }
    }
    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|cat| cat.id() == id)
            .unwrap_or_default()
    }
    pub fn name(self, lang: Lang) -> &'static str {
        i18n::cat_name(lang, self)
    }
    pub fn description(self, lang: Lang) -> &'static str {
        i18n::cat_description(lang, self)
    }
    pub fn persona(self, lang: Lang) -> &'static str {
        i18n::cat_persona(lang, self)
    }
    pub fn copy_duration(self) -> f32 {
        [
            1.06, 1.02, 0.83, 0.91, 1.13, 1.19, 0.96, 1.01, 0.88, 1.15, 1.07, 0.94,
        ][self as usize]
    }
    /// Midpoint of the open-mouth pose, leaving a full tick around capture.
    /// Midpoint of the open-mouth pose, leaving a full tick around capture.
    pub fn catch_progress(self) -> f32 {
        let ends = COPY_ENDS[self as usize];
        let slot = if matches!(self, Self::Birman | Self::Munchkin) {
            6
        } else {
            5
        };
        (ends[slot - 1] + ends[slot]) * 0.5
    }
    pub fn frame(self, copy: Option<f32>, happy: f32, sleep: f32, blink: f32) -> usize {
        if let Some(p) = copy {
            let p = p.clamp(0., 1.);
            return COPY_ENDS[self as usize]
                .iter()
                .position(|end| p < *end)
                .unwrap_or(10)
                + 1;
        }
        if sleep > 0.7 {
            15
        } else if happy > 0.55 {
            14
        } else if happy > 0.3 {
            9
        } else if blink > 0.8 {
            13
        } else if blink > 0.2 || sleep > 0.3 {
            12
        } else {
            0
        }
    }
    pub fn mouth(self, frame: usize) -> (f32, f32) {
        MOUTHS[self as usize][frame.min(15)]
    }
    /// Fish enters beside the paws and stays below the face on its approach.
    pub fn fish_start(self) -> (f32, f32) {
        let frame = self.frame(Some(self.catch_progress()), 0., 0., 0.);
        let (_, mouth_y) = self.mouth(frame);
        (228., (mouth_y + 15.).max(210.))
    }
}
const COPY_ENDS: [[f32; 11]; 12] = [
    [
        0.08, 0.16, 0.26, 0.35, 0.43, 0.51, 0.6, 0.69, 0.78, 0.87, 1.0,
    ],
    [
        0.1, 0.21, 0.31, 0.39, 0.46, 0.54, 0.61, 0.72, 0.82, 0.91, 1.0,
    ],
    [
        0.04, 0.09, 0.15, 0.23, 0.32, 0.4, 0.49, 0.63, 0.77, 0.9, 1.0,
    ],
    [
        0.06, 0.14, 0.22, 0.3, 0.39, 0.47, 0.56, 0.67, 0.79, 0.9, 1.0,
    ],
    [
        0.09, 0.19, 0.3, 0.4, 0.49, 0.58, 0.66, 0.75, 0.84, 0.92, 1.0,
    ],
    [
        0.12, 0.24, 0.35, 0.45, 0.53, 0.6, 0.67, 0.75, 0.83, 0.92, 1.0,
    ],
    [
        0.05, 0.12, 0.2, 0.29, 0.38, 0.48, 0.58, 0.69, 0.8, 0.91, 1.0,
    ],
    [
        0.07, 0.15, 0.24, 0.32, 0.42, 0.52, 0.62, 0.72, 0.81, 0.9, 1.0,
    ],
    [
        0.06, 0.13, 0.21, 0.28, 0.36, 0.45, 0.55, 0.66, 0.78, 0.89, 1.0,
    ],
    [
        0.11, 0.23, 0.34, 0.44, 0.52, 0.59, 0.65, 0.73, 0.82, 0.91, 1.0,
    ],
    [0.1, 0.2, 0.28, 0.36, 0.44, 0.53, 0.62, 0.71, 0.8, 0.9, 1.0],
    [
        0.08, 0.18, 0.27, 0.34, 0.4, 0.48, 0.57, 0.69, 0.81, 0.92, 1.0,
    ],
];
// Measured on final 240×180 release cells; y includes the 56px pet offset.
// Measured on final 240×180 release cells; y includes the 56px pet offset.
const MOUTHS: [[(f32, f32); 16]; 12] = [
    // ragdoll
    [
        (155., 169.),
        (163., 170.),
        (160., 176.),
        (158., 204.),
        (168., 207.),
        (168., 206.),
        (173., 198.),
        (170., 201.),
        (162., 175.),
        (160., 174.),
        (158., 177.),
        (158., 175.),
        (154., 181.),
        (150., 185.),
        (149., 178.),
        (159., 215.),
    ],
    // birman
    [
        (181., 152.),
        (186., 151.),
        (194., 164.),
        (203., 181.),
        (190., 149.),
        (196., 155.),
        (190., 165.),
        (183., 143.),
        (188., 145.),
        (190., 146.),
        (190., 153.),
        (190., 152.),
        (187., 154.),
        (181., 156.),
        (183., 153.),
        (185., 213.),
    ],
    // siamese
    [
        (197., 135.),
        (190., 134.),
        (204., 166.),
        (210., 188.),
        (190., 159.),
        (188., 209.),
        (196., 171.),
        (193., 128.),
        (187., 143.),
        (194., 145.),
        (202., 143.),
        (200., 141.),
        (190., 139.),
        (191., 138.),
        (190., 138.),
        (195., 217.),
    ],
    // bombay
    [
        (210., 159.),
        (207., 175.),
        (214., 202.),
        (213., 210.),
        (204., 207.),
        (205., 214.),
        (194., 207.),
        (202., 142.),
        (202., 191.),
        (209., 158.),
        (194., 169.),
        (211., 166.),
        (197., 167.),
        (190., 171.),
        (190., 170.),
        (178., 217.),
    ],
    // ragamuffin
    [
        (173., 146.),
        (193., 142.),
        (199., 188.),
        (204., 184.),
        (197., 164.),
        (184., 175.),
        (175., 180.),
        (181., 138.),
        (183., 147.),
        (179., 155.),
        (184., 145.),
        (185., 146.),
        (178., 158.),
        (176., 155.),
        (178., 146.),
        (176., 213.),
    ],
    // persian
    [
        (180., 164.),
        (193., 147.),
        (204., 182.),
        (205., 189.),
        (192., 169.),
        (199., 188.),
        (195., 140.),
        (190., 123.),
        (188., 145.),
        (173., 168.),
        (183., 155.),
        (183., 160.),
        (180., 181.),
        (183., 178.),
        (180., 173.),
        (170., 211.),
    ],
    // turkish-angora
    [
        (187., 124.),
        (191., 124.),
        (214., 167.),
        (214., 158.),
        (182., 118.),
        (190., 140.),
        (180., 126.),
        (185., 120.),
        (190., 110.),
        (199., 134.),
        (190., 123.),
        (192., 123.),
        (187., 137.),
        (192., 132.),
        (184., 126.),
        (166., 208.),
    ],
    // american-curl
    [
        (192., 134.),
        (196., 131.),
        (215., 190.),
        (216., 204.),
        (192., 177.),
        (190., 193.),
        (184., 190.),
        (198., 136.),
        (190., 129.),
        (189., 156.),
        (189., 125.),
        (192., 126.),
        (197., 136.),
        (191., 134.),
        (186., 136.),
        (167., 204.),
    ],
    // munchkin
    [
        (203., 156.),
        (190., 158.),
        (190., 189.),
        (201., 205.),
        (186., 172.),
        (185., 176.),
        (168., 190.),
        (184., 151.),
        (184., 126.),
        (190., 148.),
        (192., 152.),
        (195., 155.),
        (199., 160.),
        (197., 158.),
        (198., 148.),
        (178., 209.),
    ],
    // british-shorthair
    [
        (180., 149.),
        (191., 148.),
        (206., 177.),
        (210., 179.),
        (187., 167.),
        (197., 171.),
        (189., 165.),
        (185., 134.),
        (176., 137.),
        (186., 153.),
        (179., 145.),
        (186., 143.),
        (180., 151.),
        (178., 147.),
        (178., 144.),
        (179., 219.),
    ],
    // russian-blue
    [
        (178., 136.),
        (187., 130.),
        (215., 167.),
        (209., 190.),
        (183., 170.),
        (187., 180.),
        (189., 164.),
        (185., 124.),
        (178., 131.),
        (165., 163.),
        (177., 139.),
        (181., 140.),
        (177., 155.),
        (176., 152.),
        (175., 144.),
        (165., 216.),
    ],
    // tuxedo
    [
        (190., 140.),
        (191., 133.),
        (205., 181.),
        (204., 205.),
        (209., 201.),
        (185., 214.),
        (194., 206.),
        (204., 133.),
        (191., 126.),
        (183., 150.),
        (201., 128.),
        (200., 140.),
        (188., 151.),
        (190., 145.),
        (190., 134.),
        (169., 215.),
    ],
];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_mouth_anchor_is_on_opaque_character_art() {
        let misses: Vec<_> = CatKind::ALL
            .into_iter()
            .filter(|&cat| {
                let frame = cat.frame(Some(cat.catch_progress()), 0., 0., 0.);
                let mut target = tiny_skia::Pixmap::new(240, 256).unwrap();
                crate::hunter_art::CatArt::load(cat).unwrap().draw(
                    &mut target,
                    frame,
                    tiny_skia::Transform::identity(),
                );
                let (x, y) = cat.mouth(frame);
                !(-5..=5).any(|dy| {
                    (-5..=5).any(|dx| {
                        target
                            .pixel((x as i32 + dx) as u32, (y as i32 + dy) as u32)
                            .is_some_and(|p| p.alpha() > 128)
                    })
                })
            })
            .map(CatKind::id)
            .collect();
        assert!(
            misses.is_empty(),
            "capture mouth misses opaque art: {misses:?}"
        );
    }
    #[test]
    fn capture_occurs_in_the_open_pose_with_a_full_tick_margin() {
        for cat in CatKind::ALL {
            let progress = cat.catch_progress();
            let frame = cat.frame(Some(progress), 0., 0., 0.);
            let expected = if matches!(cat, CatKind::Birman | CatKind::Munchkin) {
                7
            } else {
                6
            };
            assert_eq!(
                frame,
                expected,
                "{} catches during its open-mouth pose",
                cat.id()
            );
            for delta in [-0.0165, 0.0165] {
                assert_eq!(
                    cat.frame(Some(progress + delta / cat.copy_duration()), 0., 0., 0.),
                    frame,
                    "{} catch pose holds across a 33ms tick",
                    cat.id()
                );
            }
        }
    }
    #[test]
    fn fish_enters_from_the_paw_region_below_the_capture_mouth() {
        for cat in CatKind::ALL {
            let (x, y) = cat.fish_start();
            let (_, mouth_y) = cat.mouth(cat.frame(Some(cat.catch_progress()), 0., 0., 0.));
            assert!(
                (210.0..=235.0).contains(&x),
                "{} fish stays inside the paw edge",
                cat.id()
            );
            assert!(
                y >= mouth_y && y < 236.,
                "{} fish approaches below the face",
                cat.id()
            );
        }
    }
    #[test]
    fn catalog_roundtrips_stable_ids_and_unknown_defaults() {
        assert_eq!(CatKind::ALL.len(), 12);
        for cat in CatKind::ALL {
            assert_eq!(CatKind::from_id(cat.id()), cat);
        }
        assert_eq!(CatKind::from_id("old-accessory"), CatKind::Ragdoll);
    }
    #[test]
    fn every_character_has_a_distinct_bounded_timeline_and_mouth_track() {
        let signatures: std::collections::HashSet<_> = CatKind::ALL
            .into_iter()
            .map(|cat| {
                assert!((0.8..=1.2).contains(&cat.copy_duration()));
                assert!((0.25..0.8).contains(&cat.catch_progress()));
                assert_eq!(cat.frame(None, 0., 0., 0.), 0);
                assert_eq!(cat.frame(None, 0., 1., 0.), 15);
                assert_eq!(cat.frame(Some(0.), 0., 0., 0.), 1, "notice is immediate");
                for frame in 0..16 {
                    let (x, y) = cat.mouth(frame);
                    assert!((0.0..240.0).contains(&x));
                    assert!((56.0..236.0).contains(&y));
                }
                (0..101)
                    .map(|n| cat.frame(Some(n as f32 / 100.), 0., 0., 0.))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(signatures.len(), 12);
    }
}
