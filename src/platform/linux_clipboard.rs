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

    pub fn name(&self, owner: Window) -> Option<String> {
        let reply = self.connection.get_property(false, owner, AtomEnum::WM_CLASS,
            AtomEnum::STRING, 0, 256).ok()?.reply().ok()?;
        let class = reply.value.split(|&b| b == 0).nth(1)?;
        (!class.is_empty()).then(|| String::from_utf8_lossy(class).into_owned())
    }
}
