//! Shared ownership gate for native flyout surfaces and their queued callbacks.

pub(super) fn dispatch<R>(owned: bool, retire: impl FnOnce(), handle: impl FnOnce() -> R) -> Option<R> {
    if owned { Some(handle()) } else { retire(); None }
}

/// Cocoa queues an initial Focused(false) while a hidden window is created.
/// Only an established keyboard session can lose focus; stale callbacks must
/// also agree with the native window's current focus state.
#[cfg(any(target_os = "macos", test))]
pub(super) fn dismiss_on_focus_loss(ever_focused: bool, has_focus: bool) -> bool {
    ever_focused && !has_focus
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{pet::Pet, state::Persist};
    use std::cell::Cell;

    #[test]
    fn initial_and_stale_focus_loss_do_not_dismiss_a_new_flyout() {
        assert!(!dismiss_on_focus_loss(false, false));
        assert!(!dismiss_on_focus_loss(true, true));
        assert!(dismiss_on_focus_loss(true, false));
    }

    #[test]
    fn ownership_transfer_retires_surface_before_cat_layout() {
        let mut pet = Pet::new(Persist::default());
        pet.open_flyout();
        let visible = Cell::new(true);
        pet.toggle_panel();
        // Both backends reconcile with this gate before cat layout/focus.
        dispatch(pet.flyout_open(), || visible.set(false), || ());
        assert!(!visible.get(), "retire the actual surface before cat focus can change");
        assert!(pet.embedded_panel_open());
    }

    #[test]
    fn delayed_flyout_callbacks_cannot_mutate_embedded_owner() {
        let mut pet = Pet::new(Persist::default());
        pet.open_flyout();
        let visible = Cell::new(true);
        let handled = dispatch(pet.flyout_open(), || visible.set(false), || pet.panel_char('a'));
        assert!(handled.is_some(), "live flyout input still dispatches");
        assert!(visible.get());
        pet.toggle_panel();
        // A delayed focus-loss callback must not close the newly embedded panel.
        let handled = dispatch(pet.flyout_open(), || visible.set(false), || pet.close_flyout());
        assert!(handled.is_none(), "retired flyout callback must not run");
        assert!(pet.embedded_panel_open());
        assert!(!visible.get());
    }
}
