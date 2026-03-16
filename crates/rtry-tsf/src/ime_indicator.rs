//! IME ONインジケーター
//!
//! IMEがONのときだけカーソル付近に小さな「あ」を表示する。

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// 現在表示中のインジケーターウィンドウハンドル
static INDICATOR_HWND: AtomicIsize = AtomicIsize::new(0);

/// IME ON状態フラグ（ウィンドウ未作成でもON状態を記録）
static INDICATOR_ACTIVE: AtomicBool = AtomicBool::new(false);

/// インジケーター表示の有効/無効（設定から制御）
static INDICATOR_ENABLED: AtomicBool = AtomicBool::new(true);

const INDICATOR_CLASS: PCWSTR = w!("RtryImeIndicator");
const INDICATOR_SIZE: i32 = 20;

/// インジケーター表示の有効/無効を設定する
pub fn set_enabled(enabled: bool) {
    INDICATOR_ENABLED.store(enabled, Ordering::SeqCst);
    if !enabled {
        dismiss();
    }
}

/// IME ON状態にする（IndicatorEditSession で位置取得後に表示される）
pub fn show() {
    INDICATOR_ACTIVE.store(true, Ordering::SeqCst);
}

/// インジケーターを閉じる
pub fn dismiss() {
    INDICATOR_ACTIVE.store(false, Ordering::SeqCst);
    let raw = INDICATOR_HWND.swap(0, Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            let _ = DestroyWindow(HWND(raw as *mut _));
        }
    }
}

/// インジケーターの位置をカーソルに追従させる（未作成なら作成する）
pub fn update_position() {
    if !INDICATOR_ACTIVE.load(Ordering::SeqCst) || !INDICATOR_ENABLED.load(Ordering::SeqCst) {
        return;
    }

    let (x, y) = crate::caret_rect::last_caret_pos();

    let raw = INDICATOR_HWND.load(Ordering::SeqCst);
    if raw != 0 {
        // 既存ウィンドウの位置を更新
        unsafe {
            let _ = SetWindowPos(
                HWND(raw as *mut _),
                None,
                x + 2,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    } else {
        // 新規作成
        unsafe {
            ensure_class_registered();
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                INDICATOR_CLASS,
                w!("あ"),
                WS_POPUP,
                x + 2,
                y,
                INDICATOR_SIZE,
                INDICATOR_SIZE,
                None,
                None,
                None,
                None,
            );
            if let Ok(hwnd) = hwnd {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let _ = UpdateWindow(hwnd);
                INDICATOR_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
            }
        }
    }
}

/// ウィンドウクラスを登録（一度だけ）
unsafe fn ensure_class_registered() {
    use std::sync::Once;
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| unsafe {
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(indicator_wnd_proc),
            hbrBackground: HBRUSH((COLOR_INFOBK.0 + 1) as *mut _),
            lpszClassName: INDICATOR_CLASS,
            ..Default::default()
        };
        RegisterClassW(&wc);
    });
}

/// ウィンドウプロシージャ
unsafe extern "system" fn indicator_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x00000000));

                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);

                let mut text: Vec<u16> = "あ".encode_utf16().collect();
                DrawTextW(
                    hdc,
                    &mut text,
                    &mut rc,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
