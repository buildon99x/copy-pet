//! Pure frame selection, independent of rendering and operating-system clocks.

pub fn frame(copy_progress: Option<f32>, happy: f32, sleep: f32, blink: f32) -> usize {
    if let Some(progress) = copy_progress {
        (progress.clamp(0.0, 0.999) * 12.0) as usize
    } else if sleep > 0.7 {
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

#[cfg(test)]
mod tests {
    use super::frame;

    #[test]
    fn copy_cycle_has_anticipation_hunt_and_recovery_instead_of_pose_jumps() {
        for (progress, expected) in [
            (0.0, 0),
            (0.1, 1),
            (0.2, 2),
            (0.3, 3),
            (0.4, 4),
            (0.5, 6),
            (0.6, 7),
            (0.7, 8),
            (0.8, 9),
            (0.9, 10),
            (0.99, 11),
        ] {
            assert_eq!(frame(Some(progress), 0.0, 0.0, 0.0), expected);
        }
        for (happy, expected) in [(0.5, 9), (0.4, 9), (0.3, 0), (0.1, 0), (0.0, 0)] {
            assert_eq!(frame(None, happy, 0.0, 0.0), expected);
        }
    }

    #[test]
    fn sleep_blink_and_affection_are_distinct_states() {
        assert_eq!(frame(None, 0.0, 1.0, 0.0), 15);
        assert_eq!(frame(None, 0.0, 0.0, 0.5), 12);
        assert_eq!(frame(None, 0.0, 0.0, 1.0), 13);
        assert_eq!(frame(None, 1.0, 0.0, 0.0), 14);
        assert_eq!(
            frame(Some(0.6), 0.0, 1.0, 1.0),
            7,
            "an accepted copy wakes the hunter"
        );
    }
}
