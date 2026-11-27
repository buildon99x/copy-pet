//! Query the X11 selection owner and its application class, never window titles.
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

pub struct SourceProbe { connection: RustConnection, selection: u32 }

impl SourceProbe {
    pub fn open() -> Option<Self> {
        let (connection, _) = x11rb::connect(None).ok()?;
        let selection = connection.intern_atom(false, b"CLIPBOARD").ok()?.reply().ok()?.atom;
        Some(Self { connection, selection })
    }

    pub fn owner(&self) -> Option<Window> {
        let id = self.connection.get_selection_owner(self.selection).ok()?.reply().ok()?.owner;
        (id != 0).then_some(id)
    }

    /// A post-write owner could already be another application. Only trust
    /// a window the X server identifies as belonging to this process.
    pub fn own_selection(&self) -> Option<Window> {
        use x11rb::protocol::res::{ClientIdMask, ClientIdSpec, ConnectionExt};
        let owner = self.owner()?;
        let spec = ClientIdSpec { client: owner, mask: ClientIdMask::LOCAL_CLIENT_PID };
        let reply = self.connection.res_query_client_ids(&[spec]).ok()?.reply().ok()?;
        reply.ids.iter().any(|id| id.spec.mask == ClientIdMask::LOCAL_CLIENT_PID
            && id.value.first() == Some(&std::process::id())).then_some(owner)
    }

    pub fn dispatch_if_owner(&self, written: Option<Window>, dispatch: impl FnOnce() -> bool) -> bool {
        written.is_some() && written == self.own_selection() && dispatch()
    }

    pub fn name(&self, owner: Window) -> Option<String> {
        let reply = self.connection.get_property(false, owner, AtomEnum::WM_CLASS,
            AtomEnum::STRING, 0, 256).ok()?.reply().ok()?;
        let class = reply.value.split(|&b| b == 0).nth(1)?;
        (!class.is_empty()).then(|| String::from_utf8_lossy(class).into_owned())
    }
}

/// Return only a viewable, externally focused window. Walking its ancestry
/// excludes ClipCat child controls, the root and X11's special focus values.
pub fn external_focus(own: Window) -> Option<Window> {
    use x11rb::protocol::xproto::MapState;
    let (connection, _) = x11rb::connect(None).ok()?;
    let focused = connection.get_input_focus().ok()?.reply().ok()?.focus;
    if focused <= 1
        || connection
            .get_window_attributes(focused)
            .ok()?
            .reply()
            .ok()?
            .map_state
            != MapState::VIEWABLE
    {
        return None;
    }
    let mut ancestor = focused;
    loop {
        if ancestor == own {
            return None;
        }
        let tree = connection.query_tree(ancestor).ok()?.reply().ok()?;
        if focused == tree.root {
            return None;
        }
        if tree.parent == tree.root {
            return Some(focused);
        }
        if tree.parent == 0 || tree.parent == ancestor {
            return None;
        }
        ancestor = tree.parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x11rb::{
        connection::Connection,
        protocol::xproto::{CreateWindowAux, InputFocus, WindowClass},
    };

    #[test]
    #[ignore = "requires a private Xvfb desktop"]
    fn x11_replacement_owner_blocks_dispatch_and_keeps_queue_ticket() {
        let mut clipboard = arboard::Clipboard::new().unwrap();
        clipboard.set_text("synthetic queued text").unwrap();
        let probe = SourceProbe::open().unwrap();
        let written = probe.own_selection();
        assert!(written.is_some());
        assert!(probe.dispatch_if_owner(written, || true));

        let (competing, screen) = x11rb::connect(None).unwrap();
        let owner = competing.generate_id().unwrap();
        competing.create_window(0, owner, competing.setup().roots[screen].root, 0, 0, 1, 1, 0,
            WindowClass::INPUT_OUTPUT, 0, &CreateWindowAux::new()).unwrap().check().unwrap();
        competing.set_selection_owner(owner, probe.selection, 0u32).unwrap().check().unwrap();
        assert_eq!(probe.owner(), Some(owner));

        let mut pet = crate::pet::Pet::new(crate::state::Persist::default());
        pet.clips = crate::clipboard::ClipStore::default();
        pet.on_copy("synthetic queued text".into(), None, None);
        pet.open_panel();
        pet.panel_nav(crate::panel::NavKey::QueueAdd);
        let pick = pet.next_queue_pick().unwrap();
        let sent = probe.dispatch_if_owner(written, || panic!("replacement selection must never be pasted"));
        pet.finish_queue_pick(sent);
        assert!(!sent);
        assert_eq!(pet.next_queue_pick(), Some(pick));
    }

    #[test]
    #[ignore = "requires a private Xvfb desktop"]
    fn x11_queue_requires_an_external_focused_window() {
        let (conn, screen) = x11rb::connect(None).unwrap();
        let root = conn.setup().roots[screen].root;
        let window = |parent| {
            let id = conn.generate_id().unwrap();
            conn.create_window(
                0,
                id,
                parent,
                0,
                0,
                20,
                20,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new(),
            )
            .unwrap()
            .check()
            .unwrap();
            conn.map_window(id).unwrap().check().unwrap();
            id
        };
        let own = window(root);
        let child = window(own);
        let external = window(root);
        for (focused, expected) in [
            (0, None),
            (1, None),
            (root, None),
            (own, None),
            (child, None),
            (external, Some(external)),
        ] {
            conn.set_input_focus(InputFocus::PARENT, focused, 0u32)
                .unwrap()
                .check()
                .unwrap();
            assert_eq!(external_focus(own), expected);
        }
    }
}
