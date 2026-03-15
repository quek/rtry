//! 候補ウィンドウ
//!
//! 交ぜ書き変換の候補をポップアップウィンドウで表示する。

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// 候補ウィンドウハンドル
static CAND_HWND: AtomicIsize = AtomicIsize::new(0);

/// 候補データ（描画用）
static CAND_DATA: Mutex<Option<CandidateData>> = Mutex::new(None);

struct CandidateData {
    candidates: Vec<String>,
    selected: usize,
}

const CAND_CLASS: PCWSTR = w!("RtryCandidateWindow");
const LINE_HEIGHT: i32 = 20;
const PADDING_X: i32 = 8;
const PADDING_Y: i32 = 4;

/// 候補ウィンドウを表示/更新
pub fn show_candidates(candidates: &[String], selected: usize) {
    // データを更新
    {
        let mut data = CAND_DATA.lock().unwrap();
        *data = Some(CandidateData {
            candidates: candidates.to_vec(),
            selected,
        });
    }

    let raw = CAND_HWND.load(Ordering::SeqCst);
    if raw != 0 {
        // 既存ウィンドウを再描画
        unsafe {
            let hwnd = HWND(raw as *mut _);
            let _ = InvalidateRect(Some(hwnd), None, true);
            let _ = UpdateWindow(hwnd);
        }
        return;
    }

    // 新規ウィンドウ作成
    unsafe {
        ensure_class_registered();

        let (x, y) = crate::stroke_help::get_caret_screen_pos();

        let count = candidates.len().min(9) as i32;
        let width = calc_max_width(candidates) + PADDING_X * 2 + 40;
        let height = count * LINE_HEIGHT + PADDING_Y * 2;

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            CAND_CLASS,
            w!(""),
            WS_POPUP | WS_BORDER,
            x,
            y + 20,
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
            CAND_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
        }
    }
}

/// 候補ウィンドウを閉じる
pub fn dismiss() {
    let raw = CAND_HWND.swap(0, Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            let _ = DestroyWindow(HWND(raw as *mut _));
        }
    }
    *CAND_DATA.lock().unwrap() = None;
}

/// テキストの最大幅を計測
unsafe fn calc_max_width(candidates: &[String]) -> i32 {
    unsafe {
        let hdc = GetDC(None);
        let mut max_w = 0i32;
        for (i, cand) in candidates.iter().enumerate().take(9) {
            let label = format!("{}. {}", i + 1, cand);
            let mut buf: Vec<u16> = label.encode_utf16().collect();
            let mut rect = RECT::default();
            DrawTextW(hdc, &mut buf, &mut rect, DT_CALCRECT | DT_NOPREFIX);
            max_w = max_w.max(rect.right - rect.left);
        }
        ReleaseDC(None, hdc);
        max_w
    }
}

/// ウィンドウクラスを登録
unsafe fn ensure_class_registered() {
    use std::sync::Once;
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| unsafe {
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(cand_wnd_proc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            lpszClassName: CAND_CLASS,
            ..Default::default()
        };
        RegisterClassW(&wc);
    });
}

/// ウィンドウプロシージャ
unsafe extern "system" fn cand_wnd_proc(
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

                let data = CAND_DATA.lock().unwrap();
                if let Some(ref data) = *data {
                    SetBkMode(hdc, TRANSPARENT);

                    for (i, cand) in data.candidates.iter().enumerate().take(9) {
                        let y = PADDING_Y + i as i32 * LINE_HEIGHT;
                        let mut rc = RECT {
                            left: 0,
                            top: y,
                            right: 1000,
                            bottom: y + LINE_HEIGHT,
                        };

                        // 選択中の候補をハイライト
                        if i == data.selected {
                            let mut fill_rc = rc;
                            let _ = GetClientRect(hwnd, &mut fill_rc);
                            fill_rc.top = y;
                            fill_rc.bottom = y + LINE_HEIGHT;
                            let brush = CreateSolidBrush(COLORREF(0x00FFD080)); // 薄い青
                            FillRect(hdc, &fill_rc, brush);
                            let _ = DeleteObject(brush.into());
                            SetTextColor(hdc, COLORREF(0x00000000));
                        } else {
                            SetTextColor(hdc, COLORREF(0x00333333));
                        }

                        let label = format!("{}. {}", i + 1, cand);
                        let mut buf: Vec<u16> = label.encode_utf16().collect();
                        rc.left = PADDING_X;
                        DrawTextW(hdc, &mut buf, &mut rc, DT_LEFT | DT_NOPREFIX | DT_SINGLELINE | DT_VCENTER);
                    }
                }

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
