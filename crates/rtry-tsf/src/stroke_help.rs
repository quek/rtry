//! ストロークヘルプ ツールチップ表示
//!
//! カーソル位置の文字のストローク（打ち方）をポップアップウィンドウで表示する。

use std::sync::atomic::{AtomicIsize, Ordering};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// 現在表示中のツールチップウィンドウハンドル（isize で保持して Send 対応）
static TOOLTIP_HWND: AtomicIsize = AtomicIsize::new(0);

const TOOLTIP_CLASS: PCWSTR = w!("RtryStrokeHelp");
const TIMER_ID: usize = 1;
const TOOLTIP_DURATION_MS: u32 = 5000;

/// ツールチップを表示
pub fn show_stroke_help(text: &str) {
    dismiss();

    let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        ensure_class_registered();

        let (x, y) = crate::caret_rect::last_caret_pos();

        // テキストサイズを計測
        let hdc = GetDC(None);
        let mut rect = RECT::default();
        let mut text_buf: Vec<u16> = text.encode_utf16().collect();
        DrawTextW(hdc, &mut text_buf, &mut rect, DT_CALCRECT | DT_NOPREFIX);
        ReleaseDC(None, hdc);

        let width = (rect.right - rect.left + 16).max(80);
        let height = (rect.bottom - rect.top + 8).max(24);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            TOOLTIP_CLASS,
            PCWSTR(text_w.as_ptr()),
            WS_POPUP | WS_BORDER,
            x,
            y + 2,
            width,
            height,
            None,
            None,
            None,
            None,
        );

        if let Ok(hwnd) = hwnd {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = UpdateWindow(hwnd);
            SetTimer(Some(hwnd), TIMER_ID, TOOLTIP_DURATION_MS, None);
            TOOLTIP_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
        }
    }
}

/// ツールチップを閉じる
pub fn dismiss() {
    let raw = TOOLTIP_HWND.swap(0, Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            let _ = DestroyWindow(HWND(raw as *mut _));
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
            lpfnWndProc: Some(tooltip_wnd_proc),
            hbrBackground: HBRUSH((COLOR_INFOBK.0 + 1) as *mut _),
            lpszClassName: TOOLTIP_CLASS,
            ..Default::default()
        };
        RegisterClassW(&wc);
    });
}

/// ウィンドウプロシージャ
unsafe extern "system" fn tooltip_wnd_proc(
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

                let len = GetWindowTextLengthW(hwnd) as usize;
                let mut buf = vec![0u16; len + 1];
                GetWindowTextW(hwnd, &mut buf);
                let text_len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());

                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x00000000));

                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                rc.left += 4;
                rc.top += 2;
                DrawTextW(hdc, &mut buf[..text_len], &mut rc, DT_LEFT | DT_NOPREFIX);

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == TIMER_ID {
                    KillTimer(Some(hwnd), TIMER_ID).ok();
                    let _ = DestroyWindow(hwnd);
                    TOOLTIP_HWND.store(0, Ordering::SeqCst);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
