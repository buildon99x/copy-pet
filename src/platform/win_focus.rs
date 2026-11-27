//! Track the identity of a focused Win32 control without reading its contents.
use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::*};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PasteTarget {
    top: HWND,
    control: HWND,
    thread: u32,
    process: u32,
}

impl PasteTarget {
    pub fn external() -> Option<Self> {
        // SAFETY: HWNDs and GUI thread metadata are validated by Win32.
        unsafe {
            let top = GetForegroundWindow();
            if top.is_null() || top == GetDesktopWindow() || top == GetShellWindow() {
                return None;
            }
            let mut process = 0;
            let thread = GetWindowThreadProcessId(top, &mut process);
            if thread == 0 || process == std::process::id() {
                return None;
            }
            let mut info: GUITHREADINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(thread, &mut info) == 0 {
                return None;
            }
            let target = Self {
                top,
                control: info.hwndFocus,
                thread,
                process,
            };
            (target.valid() && top == GetForegroundWindow()).then_some(target)
        }
    }

    #[cfg(not(feature = "portable"))]
    pub fn for_menu() -> Option<Self> {
        let target = Self::external()?;
        // Explorer may already own focus when it forwards the notification icon gesture.
        let mut name = [0u16; 128];
        let n = unsafe { GetClassNameW(target.top, name.as_mut_ptr(), name.len() as i32) };
        let class = String::from_utf16_lossy(&name[..n as usize]);
        (n > 0 && !matches!(class.as_str(), "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" |
            "NotifyIconOverflowWindow" | "TopLevelWindowForOverflowXamlIsland")).then_some(target)
    }

    #[cfg(all(test, not(feature = "portable")))]
    pub fn control_for_test(self) -> HWND {
        self.control
    }

    pub fn valid(self) -> bool {
        unsafe {
            let mut process = 0;
            !self.control.is_null()
                && IsWindow(self.control) != 0
                && GetWindowThreadProcessId(self.top, &mut process) == self.thread
                && process == self.process
                && GetAncestor(self.control, GA_ROOT) == self.top
        }
    }

    #[cfg(not(feature = "portable"))]
    pub fn restore(self) -> bool {
        use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
        if !self.valid() {
            return false;
        }
        unsafe {
            // The dedicated shortcut arrives while the destination still owns
            // focus. Never redirect focus if its control changed in the interim.
            if GetForegroundWindow() == self.top {
                return Self::external() == Some(self);
            }
            let caller = GetCurrentThreadId();
            let attached = self.thread != caller && AttachThreadInput(caller, self.thread, 1) != 0;
            if self.thread != caller && !attached {
                return false;
            }
            if SetForegroundWindow(self.top) != 0
                && GetForegroundWindow() == self.top
                && self.valid()
            {
                SetFocus(self.control);
            }
            if attached {
                AttachThreadInput(caller, self.thread, 0);
            }
            // Detaching can leave activation work queued on the foreign GUI
            // thread. Give it a bounded turn, then recheck the exact control.
            let mut result = 0;
            SendMessageTimeoutW(self.top, WM_NULL, 0, 0, SMTO_ABORTIFHUNG | SMTO_BLOCK,
                200, &mut result) != 0 && Self::external() == Some(self)
        }
    }
}
