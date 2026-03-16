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
const PAGE_SIZE: usize = 9;

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
        // 既存ウィンドウのサイズを更新して再描画
        unsafe {
            let hwnd = HWND(raw as *mut _);
            let page = selected / PAGE_SIZE;
            let page_start = page * PAGE_SIZE;
            let page_candidates =
                &candidates[page_start..candidates.len().min(page_start + PAGE_SIZE)];
            let total_pages = (candidates.len() + PAGE_SIZE - 1) / PAGE_SIZE;
            let count = page_candidates.len() as i32;
            let indicator_height = if total_pages > 1 { LINE_HEIGHT } else { 0 };
            let width = calc_page_width(page_candidates, page_start) + PADDING_X * 2 + 40;
            let height = count * LINE_HEIGHT + PADDING_Y * 2 + indicator_height;
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let _ = InvalidateRect(Some(hwnd), None, true);
            let _ = UpdateWindow(hwnd);
        }
        return;
    }

    // 新規ウィンドウ作成
    unsafe {
        ensure_class_registered();

        let (x, y) = crate::caret_rect::last_caret_pos();

        let page = selected / PAGE_SIZE;
        let page_start = page * PAGE_SIZE;
        let page_candidates = &candidates[page_start..candidates.len().min(page_start + PAGE_SIZE)];
        let total_pages = (candidates.len() + PAGE_SIZE - 1) / PAGE_SIZE;
        let count = page_candidates.len() as i32;
        // 複数ページなら下部にページインジケータ行を追加
        let indicator_height = if total_pages > 1 { LINE_HEIGHT } else { 0 };
        let width = calc_page_width(page_candidates, page_start) + PADDING_X * 2 + 40;
        let height = count * LINE_HEIGHT + PADDING_Y * 2 + indicator_height;

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            CAND_CLASS,
            w!(""),
            WS_POPUP | WS_BORDER,
            x,
            y,
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

/// 選択インデックスのみ更新して再描画（候補リストの clone 不要）
pub fn update_selected(selected: usize) {
    {
        let mut data = CAND_DATA.lock().unwrap();
        if let Some(ref mut data) = *data {
            data.selected = selected;
        } else {
            return;
        }
    }

    let raw = CAND_HWND.load(Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            let hwnd = HWND(raw as *mut _);
            let data = CAND_DATA.lock().unwrap();
            if let Some(ref data) = *data {
                let page = selected / PAGE_SIZE;
                let page_start = page * PAGE_SIZE;
                let page_candidates =
                    &data.candidates[page_start..data.candidates.len().min(page_start + PAGE_SIZE)];
                let total_pages = (data.candidates.len() + PAGE_SIZE - 1) / PAGE_SIZE;
                let count = page_candidates.len() as i32;
                let indicator_height = if total_pages > 1 { LINE_HEIGHT } else { 0 };
                let width =
                    calc_page_width(page_candidates, page_start) + PADDING_X * 2 + 40;
                let height = count * LINE_HEIGHT + PADDING_Y * 2 + indicator_height;
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    width,
                    height,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            let _ = InvalidateRect(Some(hwnd), None, true);
            let _ = UpdateWindow(hwnd);
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

/// ページ内候補テキストの最大幅を計測
unsafe fn calc_page_width(page_candidates: &[String], page_start: usize) -> i32 {
    unsafe {
        let hdc = GetDC(None);
        let mut max_w = 0i32;
        for (i, cand) in page_candidates.iter().enumerate() {
            let label = format!("{}. {}", i + 1, cand);
            let mut buf: Vec<u16> = label.encode_utf16().collect();
            let mut rect = RECT::default();
            DrawTextW(hdc, &mut buf, &mut rect, DT_CALCRECT | DT_NOPREFIX);
            max_w = max_w.max(rect.right - rect.left);
        }
        // ページインジケータの幅も考慮
        let total = page_start + page_candidates.len(); // 少なくともこのページ分はある
        let indicator = format!("[{}/{}]", page_start / PAGE_SIZE + 1, (total + PAGE_SIZE - 1) / PAGE_SIZE);
        let mut buf: Vec<u16> = indicator.encode_utf16().collect();
        let mut rect = RECT::default();
        DrawTextW(hdc, &mut buf, &mut rect, DT_CALCRECT | DT_NOPREFIX);
        max_w = max_w.max(rect.right - rect.left);
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

                    let page = data.selected / PAGE_SIZE;
                    let page_start = page * PAGE_SIZE;
                    let page_end = data.candidates.len().min(page_start + PAGE_SIZE);
                    let total_pages = (data.candidates.len() + PAGE_SIZE - 1) / PAGE_SIZE;

                    for (i, cand) in data.candidates[page_start..page_end].iter().enumerate()
                    {
                        let y = PADDING_Y + i as i32 * LINE_HEIGHT;
                        let mut rc = RECT {
                            left: 0,
                            top: y,
                            right: 1000,
                            bottom: y + LINE_HEIGHT,
                        };

                        let abs_index = page_start + i;
                        // 選択中の候補をハイライト
                        if abs_index == data.selected {
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

                    // ページインジケータ（複数ページ時のみ）
                    if total_pages > 1 {
                        let iy = PADDING_Y + (page_end - page_start) as i32 * LINE_HEIGHT;
                        let indicator = format!("[{}/{}]", page + 1, total_pages);
                        let mut buf: Vec<u16> = indicator.encode_utf16().collect();
                        SetTextColor(hdc, COLORREF(0x00888888));
                        let mut rc = RECT {
                            left: PADDING_X,
                            top: iy,
                            right: 1000,
                            bottom: iy + LINE_HEIGHT,
                        };
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
