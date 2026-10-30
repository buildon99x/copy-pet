//! Pure frame selection, independent of rendering and operating-system clocks.

pub fn frame(copy_progress: Option<f32>, happy: f32, sleep: f32, blink: f32) -> usize {
    crate::cat::CatKind::Ragdoll.frame(copy_progress, happy, sleep, blink)
}

#[cfg(test)]
mod tests {
    use super::frame;

    #[test]
    fn copy_cycle_has_anticipation_hunt_and_recovery_instead_of_pose_jumps() {
        for (progress, expected) in [
            (0.0, 1),
            (0.1, 2),
            (0.2, 3),
            (0.3, 4),
            (0.4, 5),
            (0.5, 6),
            (0.6, 8),
            (0.7, 9),
            (0.8, 10),
            (0.9, 11),
            (0.99, 11),
        ] {
            assert_eq!(frame(Some(progress), 0.0, 0.0, 0.0), expected);
        }
        for (happy, expected) in [(0.5, 9), (0.4, 9), (0.3, 0), (0.1, 0), (0.0, 0)] {
            assert_eq!(frame(None, happy, 0.0, 0.0), expected);
        }
    }

    #[test]
    fn sleep_blink_and_satisfaction_are_distinct_states() {
        assert_eq!(frame(None, 0.0, 1.0, 0.0), 15);
        assert_eq!(frame(None, 0.0, 0.0, 0.5), 12);
        assert_eq!(frame(None, 0.0, 0.0, 1.0), 13);
        assert_eq!(frame(None, 1.0, 0.0, 0.0), 14);
        assert_eq!(
            frame(Some(0.6), 0.0, 1.0, 1.0),
            8,
            "an accepted copy wakes the hunter"
        );
    }
}
