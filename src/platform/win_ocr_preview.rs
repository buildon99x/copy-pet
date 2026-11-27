//! Owned modal Windows preview for recognized text.
use std::ptr::null;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::{GetStockObject, DEFAULT_GUI_FONT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const EDIT_ID: i32 = 100;
const EM_SETLIMITTEXT: u32 = 0x00c5;
#[cfg(test)]
const EM_GETSEL: u32 = 0x00b0;
#[cfg(test)]
const EM_SETSEL: u32 = 0x00b1;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn edit_text(text: &str) -> Vec<u16> {
    // Multiline EDIT expects CRLF. Keep embedded NULs visible instead of
    // letting SetWindowTextW truncate the rest of the preview.
    wide(
        &text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\n', "\r\n")
            .replace('\0', "�"),
    )
}

unsafe fn create_edit(host: HWND, text: &[u16]) -> Option<HWND> {
    let edit = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        wide("EDIT").as_ptr(),
        null(),
        WS_CHILD
            | WS_VISIBLE
            | WS_TABSTOP
            | WS_VSCROLL
            | ES_AUTOVSCROLL as u32
            | ES_MULTILINE as u32
            | ES_READONLY as u32
            | ES_NOHIDESEL as u32,
        0,
        0,
        1,
        1,
        host,
        EDIT_ID as usize as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    if edit.is_null() {
        return None;
    }
    SendMessageW(edit, EM_SETLIMITTEXT, text.len(), 0);
    SendMessageW(
        edit,
        WM_SETFONT,
        GetStockObject(DEFAULT_GUI_FONT) as usize,
        0,
    );
    if SetWindowTextW(edit, text.as_ptr()) == 0 {
        DestroyWindow(edit);
        return None;
    }
    resize(host, edit);
    Some(edit)
}

unsafe fn resize(host: HWND, edit: HWND) {
    let mut rect: RECT = std::mem::zeroed();
    if GetClientRect(host, &mut rect) != 0 {
        MoveWindow(
            edit,
            4,
            4,
            (rect.right - 8).max(1),
            (rect.bottom - 8).max(1),
            1,
        );
    }
}

struct Content {
    title: Vec<u16>,
    text: Vec<u16>,
}

unsafe extern "system" fn dialog_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> isize {
    match msg {
        WM_INITDIALOG => {
            // DialogBoxIndirectParamW is synchronous. The caller retains Content
            // throughout WM_INITDIALOG and the EDIT control copies its text.
            let content = &*(lp as *const Content);
            SetWindowTextW(hwnd, content.title.as_ptr());
            if let Some(edit) = create_edit(hwnd, &content.text) {
                SetFocus(edit);
            } else {
                EndDialog(hwnd, -1);
            }
            0 // assigned the initial focus explicitly
        }
        WM_SIZE => {
            let edit = GetDlgItem(hwnd, EDIT_ID);
            if !edit.is_null() {
                resize(hwnd, edit);
            }
            1
        }
        WM_CLOSE => {
            EndDialog(hwnd, 1);
            1
        }
        WM_COMMAND if (wp & 0xffff) == IDCANCEL as usize => {
            EndDialog(hwnd, 1);
            1
        }
        _ => 0,
    }
}

#[repr(C, align(4))]
struct Template {
    dialog: DLGTEMPLATE,
    // Zero means no menu, default dialog class, empty initial caption.
    trailing: [u16; 3],
}

pub(super) unsafe fn show(owner: HWND, title: &str, text: &str) -> bool {
    let template = Template {
        dialog: DLGTEMPLATE {
            style: WS_POPUP
                | WS_CAPTION
                | WS_SYSMENU
                | WS_THICKFRAME
                | DS_MODALFRAME as u32
                | DS_CENTER as u32,
            dwExtendedStyle: 0,
            cdit: 0,
            x: 0,
            y: 0,
            cx: 310,
            cy: 180,
        },
        trailing: [0; 3],
    };
    let content = Content {
        title: wide(title),
        text: edit_text(text),
    };
    DialogBoxIndirectParamW(
        GetModuleHandleW(null()),
        &template.dialog,
        owner,
        Some(dialog_proc),
        &content as *const Content as LPARAM,
    ) == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_control_keeps_long_text_selectable_and_scrollable() {
        unsafe {
            let host = CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                0,
                0,
                460,
                240,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            assert!(!host.is_null());
            let text = "한 line\n".repeat(6000);
            let expected = edit_text(&text);
            let edit = create_edit(host, &expected).unwrap();
            assert_eq!(GetWindowTextLengthW(edit) as usize, expected.len() - 1);
            let style = GetWindowLongW(edit, GWL_STYLE) as u32;
            assert_eq!(
                style & (WS_VSCROLL | ES_MULTILINE as u32 | ES_READONLY as u32),
                WS_VSCROLL | ES_MULTILINE as u32 | ES_READONLY as u32
            );
            SendMessageW(edit, EM_SETSEL, 36000, 36004);
            let (mut start, mut end) = (0u32, 0u32);
            SendMessageW(
                edit,
                EM_GETSEL,
                (&mut start as *mut u32) as usize,
                (&mut end as *mut u32) as isize,
            );
            assert_eq!((start, end), (36000, 36004));
            DestroyWindow(host);
        }
    }
}
