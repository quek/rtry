//! キャレット位置キャッシュ
//!
//! EditSession 内で ITfContextView::GetTextExt を使ってキャレットの
//! スクリーン座標を取得し、グローバルにキャッシュする。
//! ポップアップウィンドウ（候補、ストロークヘルプ、IMEインジケーター）は
//! このキャッシュを参照して配置する。

use std::sync::Mutex;

use windows::core::BOOL;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// 最後に取得したキャレット矩形（スクリーン座標）
static LAST_CARET_RECT: Mutex<RECT> = Mutex::new(RECT {
    left: 0,
    top: 0,
    right: 0,
    bottom: 0,
});

/// EditSession 内からキャレット位置を更新する
///
/// GetSelection で選択範囲を取得し、GetTextExt でスクリーン座標に変換する。
/// 失敗時は GetCaretPos + ClientToScreen にフォールバックする。
pub fn update_caret_rect(ec: u32, context: &ITfContext) {
    unsafe {
        if try_update_via_text_ext(ec, context) {
            return;
        }
        // フォールバック: GetCaretPos + ClientToScreen
        update_via_caret_pos();
    }
}

/// GetTextExt でキャレット位置を取得（成功時 true）
unsafe fn try_update_via_text_ext(ec: u32, context: &ITfContext) -> bool {
    unsafe {
        // 選択範囲を取得
        let mut sel = [TF_SELECTION::default()];
        let mut fetched = 0u32;
        if context
            .GetSelection(ec, TF_DEFAULT_SELECTION, &mut sel, &mut fetched)
            .is_err()
            || fetched == 0
        {
            return false;
        }
        let range = match std::mem::ManuallyDrop::into_inner(sel[0].range.clone()) {
            Some(r) => r,
            None => return false,
        };

        // ITfContextView を取得
        let view = match context.GetActiveView() {
            Ok(v) => v,
            Err(_) => return false,
        };

        // GetTextExt でスクリーン座標を取得
        let mut rc = RECT::default();
        let mut clipped = BOOL::default();
        if view.GetTextExt(ec, &range, &mut rc, &mut clipped).is_err() {
            return false;
        }

        // CUAS 異常値ガード（tsf-tutcode 方式）
        // CUAS 互換レイヤーが {left, top, left+1, top} のような矩形を返す場合は無視
        if rc.top == rc.bottom && (rc.right - rc.left) == 1 {
            return false;
        }

        // ゼロ矩形も無視
        if rc.left == 0 && rc.top == 0 && rc.right == 0 && rc.bottom == 0 {
            return false;
        }

        *LAST_CARET_RECT.lock().unwrap() = rc;
        true
    }
}

/// GetCaretPos + ClientToScreen によるフォールバック
unsafe fn update_via_caret_pos() {
    unsafe {
        let mut pt = POINT::default();
        if GetCaretPos(&mut pt).is_ok() {
            let fg = GetForegroundWindow();
            if !fg.is_invalid() {
                let _ = ClientToScreen(fg, &mut pt);
            }
            // POINT → RECT（高さ0の矩形として格納）
            let mut rc = *LAST_CARET_RECT.lock().unwrap();
            rc.left = pt.x;
            rc.top = pt.y;
            rc.right = pt.x;
            rc.bottom = pt.y;
            *LAST_CARET_RECT.lock().unwrap() = rc;
        }
    }
}

/// キャッシュされたキャレット位置を返す (left, bottom)
///
/// テキストの左下端を返すので、ポップアップはこの位置の直下に配置する。
pub fn last_caret_pos() -> (i32, i32) {
    let rc = *LAST_CARET_RECT.lock().unwrap();
    (rc.left, rc.bottom)
}
