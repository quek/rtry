#![windows_subsystem = "windows"]

use std::ffi::c_void;
use std::mem;

use rtry_core::config::Config;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const WINDOW_CLASS: PCWSTR = w!("RtryConfigWindow");
const WINDOW_WIDTH: i32 = 320;
const WINDOW_HEIGHT: i32 = 150;

const ID_INDICATOR_CHECK: i32 = 101;
const ID_OK: i32 = 102;
const ID_CANCEL: i32 = 103;

fn main() {
    let config = Config::load();

    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH((COLOR_3DFACE.0 + 1) as *mut _),
            lpszClassName: WINDOW_CLASS,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let config_ptr = Box::into_raw(Box::new(config));

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            WINDOW_CLASS,
            w!("rtry 設定"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            None,
            None,
            Some(instance),
            Some(config_ptr as *const c_void),
        )
        .expect("CreateWindowExW failed");

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

/// システムフォント（メッセージ用）を取得
unsafe fn get_system_font() -> HFONT {
    unsafe {
        let mut metrics: NONCLIENTMETRICSW = mem::zeroed();
        metrics.cbSize = mem::size_of::<NONCLIENTMETRICSW>() as u32;

        let _ = SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            Some(&mut metrics as *mut _ as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );

        CreateFontIndirectW(&metrics.lfMessageFont)
    }
}

/// 子コントロールにフォントを設定
unsafe fn set_font(hwnd: HWND, hfont: HFONT) {
    unsafe {
        SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(hfont.0 as usize)),
            Some(LPARAM(1)),
        );
    }
}

/// コントロールを作成
unsafe fn create_controls(hwnd: HWND, config: &Config) {
    unsafe {
        let instance = HINSTANCE(GetWindowLongPtrW(hwnd, GWL_HINSTANCE) as *mut _);
        let hfont = get_system_font();

        // チェックボックス: IME ONインジケーターを表示
        let check = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("IME ONインジケーターを表示"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_AUTOCHECKBOX as u32),
            20,
            20,
            260,
            24,
            Some(hwnd),
            Some(HMENU(ID_INDICATOR_CHECK as *mut _)),
            Some(instance),
            None,
        )
        .expect("CreateWindowExW checkbox failed");
        set_font(check, hfont);

        if config.show_ime_indicator {
            SendMessageW(check, BM_SETCHECK, Some(WPARAM(BST_CHECKED.0 as usize)), None);
        }

        // OKボタン
        let ok_btn = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("OK"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_DEFPUSHBUTTON as u32),
            100,
            70,
            80,
            30,
            Some(hwnd),
            Some(HMENU(ID_OK as *mut _)),
            Some(instance),
            None,
        )
        .expect("CreateWindowExW OK button failed");
        set_font(ok_btn, hfont);

        // キャンセルボタン
        let cancel_btn = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("キャンセル"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0),
            190,
            70,
            80,
            30,
            Some(hwnd),
            Some(HMENU(ID_CANCEL as *mut _)),
            Some(instance),
            None,
        )
        .expect("CreateWindowExW Cancel button failed");
        set_font(cancel_btn, hfont);
    }
}

/// コントロールから設定を読み取って保存
unsafe fn save_config(hwnd: HWND) {
    unsafe {
        let config_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Config;
        if config_ptr.is_null() {
            return;
        }
        let config = &mut *config_ptr;

        let check = GetDlgItem(Some(hwnd), ID_INDICATOR_CHECK);
        if let Ok(check) = check {
            let state = SendMessageW(check, BM_GETCHECK, None, None);
            config.show_ime_indicator = state.0 == BST_CHECKED.0 as isize;
        }

        let _ = config.save();
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                let config_ptr = cs.lpCreateParams as *mut Config;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, config_ptr as isize);
                create_controls(hwnd, &*config_ptr);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                match id {
                    ID_OK => {
                        save_config(hwnd);
                        let _ = DestroyWindow(hwnd);
                    }
                    ID_CANCEL => {
                        let _ = DestroyWindow(hwnd);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                // Config を解放
                let config_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Config;
                if !config_ptr.is_null() {
                    let _ = Box::from_raw(config_ptr);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
