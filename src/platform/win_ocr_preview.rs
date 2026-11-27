//! Owned modal Windows preview for recognized text.
use crate::i18n::{image_text_message, ImageTextMessage, Lang};
use std::ptr::null;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::{GetStockObject, DEFAULT_GUI_FONT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const EDIT_ID: i32 = 100;
const EM_SETLIMITTEXT: u32 = 0x00c5;
const EM_GETSEL: u32 = 0x00b0;
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

/// The caller keeps Content alive until its modal dialog and this EDIT are destroyed.
unsafe fn create_edit(host: HWND, content: &Content) -> Option<HWND> {
    let text = &content.text;
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
    if SetWindowSubclass(
        edit,
        Some(selection_proc),
        1,
        content as *const Content as usize,
    ) == 0
    {
        DestroyWindow(edit);
        return None;
    }
    resize(host, edit);
    Some(edit)
}

unsafe fn copy_selected_range(edit: HWND, displayed: &[u16], copy: impl FnOnce(String)) {
    let (mut start, mut end) = (0u32, 0u32);
    SendMessageW(
        edit,
        EM_GETSEL,
        &mut start as *mut u32 as usize,
        &mut end as *mut u32 as isize,
    );
    if start >= end || end as usize >= displayed.len() {
        return;
    }
    copy(String::from_utf16_lossy(
        &displayed[start as usize..end as usize],
    ));
}

unsafe fn selection_menu(edit: HWND, pos: LPARAM, content: &Content) {
    const COPY: usize = 1;
    const SELECT_ALL: usize = 2;
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return;
    }
    let mut point = POINT {
        x: pos as i16 as i32,
        y: (pos >> 16) as i16 as i32,
    };
    if pos == -1 {
        GetCaretPos(&mut point);
        windows_sys::Win32::Graphics::Gdi::ClientToScreen(edit, &mut point);
    }
    let copy_label = wide(image_text_message(
        content.lang,
        ImageTextMessage::CopySelection,
    ));
    let all_label = wide(image_text_message(
        content.lang,
        ImageTextMessage::SelectAll,
    ));
    AppendMenuW(menu, MF_STRING, COPY, copy_label.as_ptr());
    AppendMenuW(menu, MF_STRING, SELECT_ALL, all_label.as_ptr());
    let choice = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
        point.x,
        point.y,
        0,
        edit,
        null(),
    ) as usize;
    DestroyMenu(menu);
    match choice {
        COPY => {
            SendMessageW(edit, WM_COPY, 0, 0);
        }
        SELECT_ALL => {
            SendMessageW(edit, EM_SETSEL, 0, -1);
        }
        _ => {}
    }
}

unsafe extern "system" fn selection_proc(
    edit: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    // SetWindowSubclass is scoped to the EDIT on its creating UI thread. Its
    // parent dialog owns the immutable Content for the full modal call.
    let content = &*(data as *const Content);
    match msg {
        WM_COPY => {
            copy_selected_range(edit, &content.text, content.copy);
            0
        }
        WM_KEYDOWN if (wp == 0x43 || wp == 0x2d) && GetKeyState(0x11) < 0 => {
            SendMessageW(edit, WM_COPY, 0, 0);
            0
        }
        // TranslateMessage also queues ETX for Ctrl+C. The keydown above
        // already handled it; avoid a second system write/notification.
        WM_CHAR if wp == 3 => 0,
        WM_CONTEXTMENU => {
            selection_menu(edit, lp, content);
            0
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(edit, Some(selection_proc), id);
            DefSubclassProc(edit, msg, wp, lp)
        }
        _ => DefSubclassProc(edit, msg, wp, lp),
    }
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
    lang: Lang,
    copy: fn(String),
    text: Vec<u16>,
}

unsafe extern "system" fn dialog_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> isize {
    match msg {
        WM_INITDIALOG => {
            // DialogBoxIndirectParamW is synchronous. The caller retains Content
            // throughout WM_INITDIALOG and the EDIT control copies its text.
            let content = &*(lp as *const Content);
            SetWindowTextW(
                hwnd,
                wide(image_text_message(content.lang, ImageTextMessage::Preview)).as_ptr(),
            );
            if let Some(edit) = create_edit(hwnd, content) {
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

pub(super) unsafe fn show(owner: HWND, lang: Lang, text: &str, copy: fn(String)) -> bool {
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
        lang,
        copy,
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
    use std::cell::RefCell;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardState, SetKeyboardState};
    thread_local! { static COPIED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }

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
            let content = Content {
                lang: Lang::En,
                text: expected.clone(),
                copy: |text| COPIED.with(|copies| copies.borrow_mut().push(text)),
            };
            let edit = create_edit(host, &content).unwrap();
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
            let selected = String::from_utf16(&expected[36000..36004]).unwrap();
            SendMessageW(edit, WM_COPY, 0, 0);
            let mut previous = [0u8; 256];
            assert_ne!(GetKeyboardState(previous.as_mut_ptr()), 0);
            let mut pressed = previous;
            pressed[0x11] |= 0x80;
            assert_ne!(SetKeyboardState(pressed.as_ptr()), 0);
            // Only this synthetic control receives the key. No global input injection.
            SendMessageW(edit, WM_KEYDOWN, 0x43, 0);
            SendMessageW(edit, WM_CHAR, 3, 0);
            assert_ne!(SetKeyboardState(previous.as_ptr()), 0);
            SendMessageW(edit, EM_SETSEL, 11, 11);
            SendMessageW(edit, WM_COPY, 0, 0);
            COPIED.with(|copies| assert_eq!(copies.take(), [selected.clone(), selected]));
            DestroyWindow(host);
        }
    }
}
