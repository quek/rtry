#![windows_subsystem = "windows"]

use std::ffi::c_void;
use std::mem;

use rtry_core::config::Config;
use rtry_core::table::QWERTY_KEYS;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const WINDOW_CLASS: PCWSTR = w!("RtryConfigWindow");
const WINDOW_WIDTH: i32 = 340;
const WINDOW_HEIGHT: i32 = 340;

const ID_INDICATOR_CHECK: i32 = 101;
const ID_OK: i32 = 102;
const ID_CANCEL: i32 = 103;
const ID_PREFIX_KEY_LABEL: i32 = 104;
const ID_PREFIX_KEY_EDIT: i32 = 105;
const ID_KEY_LAYOUT_LABEL: i32 = 106;
const ID_KEY_LAYOUT_EDIT: i32 = 107;

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

/// 等幅フォントを取得
unsafe fn get_monospace_font() -> HFONT {
    unsafe {
        CreateFontW(
            16,     // height
            0,      // width
            0,      // escapement
            0,      // orientation
            FW_NORMAL.0 as i32,
            0, 0, 0, // italic, underline, strikeout
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            (FF_MODERN.0 | FIXED_PITCH.0) as u32,
            w!("Consolas"),
        )
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

/// キーレイアウトを表示用文字列に変換（4行×10列、スペース区切り）
fn key_layout_to_string(keys: &[char; 40]) -> String {
    let mut s = String::new();
    for row in 0..4 {
        if row > 0 {
            s.push_str("\r\n");
        }
        for col in 0..10 {
            if col > 0 {
                s.push(' ');
            }
            s.push(keys[row * 10 + col]);
        }
    }
    s
}

/// 表示用文字列からキーレイアウトをパース
fn parse_key_layout(text: &str) -> Option<[char; 40]> {
    let chars: Vec<char> = text
        .split(|c: char| c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| {
            let mut chars = s.chars();
            let ch = chars.next()?;
            if chars.next().is_some() {
                None // 2文字以上のトークンは無効
            } else {
                Some(ch)
            }
        })
        .collect();

    if chars.len() != 40 {
        return None;
    }

    let mut arr = [' '; 40];
    arr.copy_from_slice(&chars);
    Some(arr)
}

/// コントロールを作成
unsafe fn create_controls(hwnd: HWND, config: &Config) {
    unsafe {
        let instance = HINSTANCE(GetWindowLongPtrW(hwnd, GWL_HINSTANCE) as *mut _);
        let hfont = get_system_font();
        let mono_font = get_monospace_font();

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

        // ラベル: 3ストロークプレフィックスキー
        let label = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("3ストロークプレフィックスキー:"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
            20,
            54,
            210,
            24,
            Some(hwnd),
            Some(HMENU(ID_PREFIX_KEY_LABEL as *mut _)),
            Some(instance),
            None,
        )
        .expect("CreateWindowExW label failed");
        set_font(label, hfont);

        // テキスト入力: プレフィックスキー（1文字）
        let prefix_text = if config.ext_prefix_key == ' ' {
            "Space"
        } else {
            // 1文字を静的文字列にはできないのでウィンドウ作成後にセット
            ""
        };
        let edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            &HSTRING::from(prefix_text),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL as u32),
            232,
            52,
            60,
            24,
            Some(hwnd),
            Some(HMENU(ID_PREFIX_KEY_EDIT as *mut _)),
            Some(instance),
            None,
        )
        .expect("CreateWindowExW edit failed");
        set_font(edit, hfont);

        // Space 以外の1文字の場合はセット
        if config.ext_prefix_key != ' ' {
            let text = HSTRING::from(config.ext_prefix_key.to_string());
            let _ = SetWindowTextW(edit, &text);
        }

        // ラベル: キーレイアウト
        let layout_label = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("キーレイアウト (4行×10列):"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
            20,
            88,
            260,
            24,
            Some(hwnd),
            Some(HMENU(ID_KEY_LAYOUT_LABEL as *mut _)),
            Some(instance),
            None,
        )
        .expect("CreateWindowExW layout label failed");
        set_font(layout_label, hfont);

        // マルチラインテキスト: キーレイアウト
        let layout_text = key_layout_to_string(&config.effective_key_layout());
        let layout_edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            &HSTRING::from(layout_text),
            WINDOW_STYLE(
                WS_CHILD.0
                    | WS_VISIBLE.0
                    | WS_TABSTOP.0
                    | WS_VSCROLL.0
                    | ES_MULTILINE as u32
                    | ES_WANTRETURN as u32,
            ),
            20,
            112,
            295,
            80,
            Some(hwnd),
            Some(HMENU(ID_KEY_LAYOUT_EDIT as *mut _)),
            Some(instance),
            None,
        )
        .expect("CreateWindowExW layout edit failed");
        set_font(layout_edit, mono_font);

        // OKボタン
        let ok_btn = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("OK"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_DEFPUSHBUTTON as u32),
            100,
            210,
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
            210,
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

/// コントロールから設定を読み取って保存。成功時 true、バリデーション失敗時 false
unsafe fn save_config(hwnd: HWND) -> bool {
    unsafe {
        let config_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Config;
        if config_ptr.is_null() {
            return false;
        }
        let config = &mut *config_ptr;

        let check = GetDlgItem(Some(hwnd), ID_INDICATOR_CHECK);
        if let Ok(check) = check {
            let state = SendMessageW(check, BM_GETCHECK, None, None);
            config.show_ime_indicator = state.0 == BST_CHECKED.0 as isize;
        }

        // プレフィックスキーの読み取り
        let edit = GetDlgItem(Some(hwnd), ID_PREFIX_KEY_EDIT);
        if let Ok(edit) = edit {
            let mut buf = [0u16; 16];
            let len = GetWindowTextW(edit, &mut buf);
            if len > 0 {
                let text = String::from_utf16_lossy(&buf[..len as usize]);
                let text = text.trim();
                if text.eq_ignore_ascii_case("space") {
                    config.ext_prefix_key = ' ';
                } else if let Some(ch) = text.chars().next() {
                    config.ext_prefix_key = ch;
                }
            }
        }

        // キーレイアウトの読み取り
        let layout_edit = GetDlgItem(Some(hwnd), ID_KEY_LAYOUT_EDIT);
        if let Ok(layout_edit) = layout_edit {
            let mut buf = [0u16; 256];
            let len = GetWindowTextW(layout_edit, &mut buf);
            if len > 0 {
                let text = String::from_utf16_lossy(&buf[..len as usize]);
                if let Some(layout) = parse_key_layout(&text) {
                    if layout == QWERTY_KEYS {
                        config.key_layout_40 = None;
                    } else {
                        config.key_layout_40 = Some(layout.to_vec());
                    }
                } else {
                    // パース失敗時はエラーメッセージを表示
                    let _ = MessageBoxW(
                        Some(hwnd),
                        w!("キーレイアウトは4行×10列の1文字ずつスペース区切りで入力してください。"),
                        w!("入力エラー"),
                        MB_OK | MB_ICONWARNING,
                    );
                    return false;
                }
            }
        }

        let _ = config.save();
        true
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
                        if save_config(hwnd) {
                            let _ = DestroyWindow(hwnd);
                        }
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
