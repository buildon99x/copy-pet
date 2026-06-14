//! Platform-agnostic context-menu model.
//!
//! The native menus (macOS `NSMenu`, and the Windows tray menu in principle)
//! render this tree; the backend applies the chosen [`MenuAction`] through
//! [`crate::pet::Pet::apply_menu_action`]. Keeping the structure, the check
//! marks and the action logic here — instead of inside a platform backend —
//! is what makes the whole menu unit- and e2e-testable without a GUI.
//!
//! [`Pet::build_menu`](crate::pet::Pet::build_menu) produces the tree with live
//! state; `apply_menu_action` performs the pure state mutations and returns a
//! [`MenuOutcome`] for the few items a backend must finish (a confirmation
//! dialog, the About box, the autostart toggle, opening the update page, quit).

use crate::i18n::Lang;

/// What a chosen menu item does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    TogglePanel,
    ToggleCapture,
    /// Close the clipboard panel automatically after copying a clip (on/off).
    TogglePanelAutoClose,
    /// Paste the picked clip into the previous app automatically (on/off).
    TogglePasteOnSelect,
    ToggleStats,
    SetSize(usize),
    SetAccessory(usize),
    SetSound(u8),
    ToggleLock,
    SetLang(Lang),
    ToggleAutostart,
    ToggleAutoUpdate,
    /// Advance the global panel hotkey to the next preset (see [`crate::hotkey`]).
    CycleHotkey,
    /// macOS opens the releases page (no in-app self-replace there).
    InstallUpdate,
    ResetStats,
    About,
    Quit,
}

/// One entry in a (sub)menu: a separator or a real item.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MenuEntry {
    Separator,
    Item(MenuItem),
}

/// A rendered menu item. A non-empty `submenu` makes it a parent and its
/// `action` is ignored; `checked` shows a check/radio mark; `enabled == false`
/// greys it out (e.g. a still-locked accessory).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MenuItem {
    pub label: String,
    pub action: Option<MenuAction>,
    pub checked: bool,
    pub enabled: bool,
    pub submenu: Vec<MenuEntry>,
}

impl MenuItem {
    /// A leaf item: an action, a check state, always enabled, no submenu.
    pub fn leaf(label: impl Into<String>, action: MenuAction, checked: bool) -> MenuEntry {
        MenuEntry::Item(MenuItem {
            label: label.into(),
            action: Some(action),
            checked,
            enabled: true,
            submenu: Vec::new(),
        })
    }

    /// A submenu parent holding `submenu`.
    pub fn parent(label: impl Into<String>, submenu: Vec<MenuEntry>) -> MenuEntry {
        MenuEntry::Item(MenuItem {
            label: label.into(),
            action: None,
            checked: false,
            enabled: true,
            submenu,
        })
    }
}

/// The backend's follow-up after [`Pet::apply_menu_action`](crate::pet::Pet::apply_menu_action).
/// Pure state changes return `Handled`; the rest need OS / dialog / lifecycle work
/// the platform-agnostic core must not do itself.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MenuOutcome {
    Handled,
    /// Confirm with the user, then call [`Pet::reset_stats`](crate::pet::Pet::reset_stats).
    ConfirmReset,
    /// Show the About dialog (`i18n::about_text`).
    ShowAbout,
    /// Flip the platform autostart registration.
    ToggleAutostart,
    /// Open the releases page / start the update.
    InstallUpdate,
    /// Re-register the OS panel hotkey from this (new) spec; the core already
    /// updated the persisted spec via [`Pet::cycle_hotkey`](crate::pet::Pet::cycle_hotkey).
    ReregisterHotkey(String),
    /// Save and exit.
    Quit,
}
