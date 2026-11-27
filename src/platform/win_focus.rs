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
        (n > 0
            && !matches!(
                class.as_str(),
                "Shell_TrayWnd"
                    | "Shell_SecondaryTrayWnd"
                    | "NotifyIconOverflowWindow"
                    | "TopLevelWindowForOverflowXamlIsland"
            ))
        .then_some(target)
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
            let original = GetForegroundWindow();
            let requested = SetForegroundWindow(self.top);
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(450);
            let mut focused = false;
            let mut adjusted = false;
            let mut previous_active = false;
            let mut info: GUITHREADINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            while std::time::Instant::now() < deadline {
                // Let both independent GUI queues handle activation/deactivation.
                // PM_NOREMOVE services sent messages without consuming user input.
                let mut msg = std::mem::zeroed();
                PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                let mut result = 0;
                if SendMessageTimeoutW(self.top, WM_NULL, 0, 0, SMTO_ABORTIFHUNG, 50, &mut result)
                    == 0
                    || !self.valid()
                {
                    break;
                }
                let fg = GetForegroundWindow();
                if !fg.is_null() && fg != original && fg != self.top {
                    break;
                }
                if GetGUIThreadInfo(self.thread, &mut info) == 0 {
                    break;
                }
                let activated = fg == self.top && info.hwndActive == self.top;
                let exact = activated && info.hwndFocus == self.control;
                if exact && focused {
                    return true;
                }
                focused = exact;
                if activated && previous_active && !exact && !adjusted {
                    // Give the frame a turn to restore its view itself first.
                    // Attach only to adjust the child of the proven active frame.
                    let caller = GetCurrentThreadId();
                    if AttachThreadInput(caller, self.thread, 1) == 0 {
                        break;
                    }
                    if GetForegroundWindow() == self.top && self.valid() {
                        SetFocus(self.control);
                    }
                    AttachThreadInput(caller, self.thread, 0);
                    adjusted = true;
                }
                previous_active = activated;
                std::thread::sleep(std::time::Duration::from_millis(8));
            }
            #[cfg(test)]
            eprintln!("restore did not settle: requested={requested}, activated={previous_active}, exact={focused}, adjusted={adjusted}, original={original:?}, foreground={:?}, active={:?}, focus={:?}, flags={:#x}",
                GetForegroundWindow(), info.hwndActive, info.hwndFocus, info.flags);
            #[cfg(not(test))]
            let _ = requested;
            false
        }
    }
}
