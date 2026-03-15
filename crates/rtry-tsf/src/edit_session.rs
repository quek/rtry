//! ITfEditSession 実装
//!
//! TSFではテキストの変更は EditSession 内でのみ許可される。
//! RequestEditSession() で登録し、TSFマネージャが DoEditSession() を呼び出す。
//!
//! SharedComposition (Arc<Mutex<>>) を通じて、EditSession内から
//! TryCodeTextServiceのcomposition状態を更新する。

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;

use std::sync::{Arc, Mutex};

use rtry_core::mazegaki::MazegakiDictionary;
use rtry_core::table::TryCodeTable;

use crate::composition::SharedComposition;
use crate::text_service::{MazegakiState, SharedPostBuf};

/// カーソルをレンジ末尾に移動するセレクションを設定
unsafe fn set_cursor_to_end(context: &ITfContext, ec: u32, range: &ITfRange) -> Result<()> {
    unsafe {
        let sel_range = range.Clone()?;
        sel_range.Collapse(ec, TF_ANCHOR_END)?;
        let selection = TF_SELECTION {
            range: std::mem::ManuallyDrop::new(Some(sel_range)),
            style: TF_SELECTIONSTYLE {
                ase: TF_AE_NONE,
                fInterimChar: false.into(),
            },
        };
        let _ = context.SetSelection(ec, &[selection]);
    }
    Ok(())
}

/// GetSelection でカーソル位置の ITfRange を取得する
unsafe fn get_selection_range(context: &ITfContext, ec: u32) -> Result<Option<ITfRange>> {
    unsafe {
        let mut sel = [TF_SELECTION::default()];
        let mut fetched = 0u32;
        context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut sel, &mut fetched)?;
        if fetched == 0 {
            return Ok(None);
        }
        let range = std::mem::ManuallyDrop::into_inner(sel[0].range.clone())
            .ok_or_else(|| Error::from_hresult(E_FAIL))?;
        Ok(Some(range))
    }
}

/// 文字列確定用のエディットセッション
#[implement(ITfEditSession)]
pub struct CommitEditSession {
    context: ITfContext,
    text: String,
    _client_id: u32,
    shared_comp: SharedComposition,
    composition_sink: ITfCompositionSink,
    postbuf: SharedPostBuf,
}

impl CommitEditSession {
    pub fn new(
        context: ITfContext,
        text: String,
        client_id: u32,
        shared_comp: SharedComposition,
        composition_sink: ITfCompositionSink,
        postbuf: SharedPostBuf,
    ) -> Self {
        CommitEditSession {
            context, text, _client_id: client_id, shared_comp, composition_sink, postbuf,
        }
    }
}

use crate::text_service::postbuf_append;

impl ITfEditSession_Impl for CommitEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        crate::caret_rect::update_caret_rect(ec, &self.context);
        unsafe {
            let composition = if let Some(comp) = self.shared_comp.take() {
                comp
            } else {
                // コンポジションがない場合は一時的に開始
                let insert_at_sel: ITfInsertAtSelection = self.context.cast()?;
                let range = insert_at_sel.InsertTextAtSelection(
                    ec,
                    TF_IAS_QUERYONLY,
                    &[],
                )?;
                let context_composition: ITfContextComposition = self.context.cast()?;
                context_composition.StartComposition(
                    ec,
                    &range,
                    &self.composition_sink,
                )?
            };

            // コンポジション範囲にテキストを設定して終了
            let range = composition.GetRange()?;
            let text_w: Vec<u16> = self.text.encode_utf16().collect();
            range.SetText(ec, TF_ST_CORRECTION, &text_w)?;
            // カーソルをテキスト末尾に移動
            set_cursor_to_end(&self.context, ec, &range)?;
            let _ = composition.EndComposition(ec);
        }
        crate::debug_log!("CommitEditSession: committed '{}'", self.text);
        postbuf_append(&self.postbuf, &self.text);
        Ok(())
    }
}

/// 合成文字列表示用のエディットセッション
#[implement(ITfEditSession)]
pub struct ComposingEditSession {
    context: ITfContext,
    text: String,
    _client_id: u32,
    shared_comp: SharedComposition,
    composition_sink: ITfCompositionSink,
}

impl ComposingEditSession {
    pub fn new(
        context: ITfContext,
        text: String,
        client_id: u32,
        shared_comp: SharedComposition,
        composition_sink: ITfCompositionSink,
    ) -> Self {
        ComposingEditSession {
            context, text, _client_id: client_id, shared_comp,
            composition_sink,
        }
    }
}

impl ITfEditSession_Impl for ComposingEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        crate::caret_rect::update_caret_rect(ec, &self.context);
        unsafe {
            if let Some(composition) = self.shared_comp.get() {
                // 既存のコンポジションを更新
                let range = composition.GetRange()?;
                let text_w: Vec<u16> = self.text.encode_utf16().collect();
                range.SetText(ec, TF_ST_CORRECTION, &text_w)?;
                crate::debug_log!("ComposingEditSession: updated to '{}'", self.text);
            } else {
                // 新しいコンポジションを開始
                let insert_at_sel: ITfInsertAtSelection = self.context.cast()?;
                let range = insert_at_sel.InsertTextAtSelection(
                    ec,
                    TF_IAS_QUERYONLY,
                    &[],
                )?;

                let context_composition: ITfContextComposition = self.context.cast()?;
                let composition = context_composition.StartComposition(
                    ec,
                    &range,
                    &self.composition_sink,
                )?;

                // テキストを設定
                let comp_range = composition.GetRange()?;
                let text_w: Vec<u16> = self.text.encode_utf16().collect();
                comp_range.SetText(ec, TF_ST_CORRECTION, &text_w)?;

                // カーソルをコンポジション末尾に移動
                set_cursor_to_end(&self.context, ec, &comp_range)?;

                // SharedCompositionに保存 → 次回のEditSessionで参照可能
                self.shared_comp.set(composition);

                crate::debug_log!("ComposingEditSession: started with '{}'", self.text);
            }
        }
        Ok(())
    }
}

/// コンポジション終了用のエディットセッション
#[implement(ITfEditSession)]
pub struct EndCompositionEditSession {
    context: ITfContext,
    _client_id: u32,
    shared_comp: SharedComposition,
}

impl EndCompositionEditSession {
    pub fn new(
        context: ITfContext,
        client_id: u32,
        shared_comp: SharedComposition,
    ) -> Self {
        EndCompositionEditSession { context, _client_id: client_id, shared_comp }
    }
}

impl ITfEditSession_Impl for EndCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        crate::caret_rect::update_caret_rect(ec, &self.context);
        unsafe {
            if let Some(composition) = self.shared_comp.take() {
                let range = composition.GetRange()?;
                range.SetText(ec, TF_ST_CORRECTION, &[])?;
                let _ = composition.EndComposition(ec);
                crate::debug_log!("EndCompositionEditSession: ended composition");
            }
        }
        Ok(())
    }
}

/// ストロークヘルプ用のエディットセッション
///
/// カーソル位置（またはその直前）の文字を読み取り、逆引きテーブルで
/// ストロークを取得してツールチップで表示する。
#[implement(ITfEditSession)]
pub struct CharHelpEditSession {
    context: ITfContext,
    table: Arc<TryCodeTable>,
    postbuf: SharedPostBuf,
}

impl CharHelpEditSession {
    pub fn new(context: ITfContext, table: Arc<TryCodeTable>, postbuf: SharedPostBuf) -> Self {
        CharHelpEditSession { context, table, postbuf }
    }
}

impl ITfEditSession_Impl for CharHelpEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        crate::caret_rect::update_caret_rect(ec, &self.context);
        unsafe {
            // カーソル位置を取得
            let Some(range) = get_selection_range(&self.context, ec)? else {
                crate::debug_log!("CharHelp: GetSelection returned 0 selections");
                return Ok(());
            };

            // カーソル位置を起点にレンジを作成（1文字前に拡張）
            let read_range = range.Clone()?;
            read_range.Collapse(ec, TF_ANCHOR_START)?;
            let mut actual = 0i32;
            read_range.ShiftStart(ec, -1, &mut actual, std::ptr::null())?;
            crate::debug_log!("CharHelp: ShiftStart(-1) shifted={}", actual);

            // テキストを読み取り
            let mut buf = [0u16; 4]; // UTF-16 で最大2ユニット + 余裕
            let mut cch = 0u32;
            read_range.GetText(ec, 0, &mut buf, &mut cch)?;

            let ch_string;
            let ch;
            if cch > 0 {
                ch_string = String::from_utf16_lossy(&buf[..cch as usize]);
                ch = ch_string.trim();
            } else {
                // CUAS環境フォールバック: postbuf の末尾1文字を使用
                let buf_content = self.postbuf.lock().unwrap();
                if let Some(last_char) = buf_content.chars().next_back() {
                    crate::debug_log!("CharHelp: TSF read failed, using postbuf fallback '{}'", last_char);
                    ch_string = last_char.to_string();
                    ch = &ch_string;
                } else {
                    crate::debug_log!("CharHelp: GetText returned 0 chars, postbuf empty");
                    return Ok(());
                }
            }
            if ch.is_empty() {
                crate::debug_log!("CharHelp: text is empty after trim");
                return Ok(());
            }

            // 逆引き
            let strokes = self.table.reverse_lookup(ch);
            if strokes.is_empty() {
                crate::debug_log!("CharHelp: no strokes found for '{}'", ch);
                let msg = format!("「{}」 ストロークなし", ch);
                crate::stroke_help::show_stroke_help(&msg);
            } else {
                let stroke_strs: Vec<String> = strokes.iter()
                    .map(|s| s.to_display_string())
                    .collect();
                let msg = format!("「{}」 {}", ch, stroke_strs.join(" / "));
                crate::debug_log!("CharHelp: '{}' → {}", ch, msg);
                crate::stroke_help::show_stroke_help(&msg);
            }
        }
        Ok(())
    }
}

/// 交ぜ書き変換開始用のエディットセッション
///
/// カーソル前のテキストを読み取り、辞書で最長一致検索し、
/// 読みの範囲をコンポジションで置換して候補ウィンドウを表示する。
#[implement(ITfEditSession)]
pub struct MazegakiStartEditSession {
    context: ITfContext,
    shared_comp: SharedComposition,
    composition_sink: ITfCompositionSink,
    dict: Arc<MazegakiDictionary>,
    result_slot: Arc<Mutex<Option<MazegakiState>>>,
    postbuf: SharedPostBuf,
}

impl MazegakiStartEditSession {
    pub(crate) fn new(
        context: ITfContext,
        shared_comp: SharedComposition,
        composition_sink: ITfCompositionSink,
        dict: Arc<MazegakiDictionary>,
        result_slot: Arc<Mutex<Option<MazegakiState>>>,
        postbuf: SharedPostBuf,
    ) -> Self {
        MazegakiStartEditSession {
            context, shared_comp, composition_sink, dict, result_slot, postbuf,
        }
    }
}

impl ITfEditSession_Impl for MazegakiStartEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        crate::caret_rect::update_caret_rect(ec, &self.context);
        unsafe {
            // カーソル位置を取得
            let Some(range) = get_selection_range(&self.context, ec)? else {
                crate::debug_log!("MazegakiStart: GetSelection returned 0 selections");
                return Ok(());
            };

            // カーソル前の最大10文字を読み取り
            let read_range = range.Clone()?;
            read_range.Collapse(ec, TF_ANCHOR_START)?;
            let mut shifted = 0i32;
            read_range.ShiftStart(ec, -10, &mut shifted, std::ptr::null())?;
            crate::debug_log!("MazegakiStart: ShiftStart(-10) shifted={}", shifted);

            let mut buf = [0u16; 20];
            let mut cch = 0u32;
            read_range.GetText(ec, 0, &mut buf, &mut cch)?;

            let text;
            let use_postbuf;
            if cch > 0 {
                text = String::from_utf16_lossy(&buf[..cch as usize]);
                use_postbuf = false;
                crate::debug_log!("MazegakiStart: text before cursor = '{}'", text);
            } else {
                // CUAS環境フォールバック: postbuf の内容を使用
                let buf_content = self.postbuf.lock().unwrap();
                if buf_content.is_empty() {
                    crate::debug_log!("MazegakiStart: GetText returned 0 chars, postbuf empty");
                    return Ok(());
                }
                text = buf_content.clone();
                use_postbuf = true;
                crate::debug_log!("MazegakiStart: TSF read failed, using postbuf fallback '{}'", text);
            }

            // 最長一致検索
            let Some((reading_len, reading, candidates)) = self.dict.find_longest_match(&text) else {
                crate::debug_log!("MazegakiStart: no match found");
                return Ok(());
            };
            crate::debug_log!("MazegakiStart: matched '{}' ({} chars), {} candidates",
                reading, reading_len, candidates.len());

            if !use_postbuf {
                // 通常環境: 読みの範囲にコンポジションを開始
                let comp_range = range.Clone()?;
                comp_range.Collapse(ec, TF_ANCHOR_START)?;
                comp_range.ShiftStart(ec, -(reading_len as i32), &mut shifted, std::ptr::null())?;

                let ctx_comp: ITfContextComposition = self.context.cast()?;
                let composition = ctx_comp.StartComposition(
                    ec,
                    &comp_range,
                    &self.composition_sink,
                )?;

                // 第一候補でテキスト置換
                let first = &candidates[0];
                let text_w: Vec<u16> = first.encode_utf16().collect();
                let comp_range2 = composition.GetRange()?;
                comp_range2.SetText(ec, TF_ST_CORRECTION, &text_w)?;

                set_cursor_to_end(&self.context, ec, &comp_range2)?;
                self.shared_comp.set(composition);
            }

            // MazegakiState を結果スロットにセット
            let state = MazegakiState {
                reading,
                candidates: candidates.to_vec(),
                selected: 0,
                postbuf_reading_len: if use_postbuf { Some(reading_len) } else { None },
            };
            *self.result_slot.lock().unwrap() = Some(state);
        }
        Ok(())
    }
}

/// 交ぜ書き候補更新用のエディットセッション
#[implement(ITfEditSession)]
pub struct MazegakiUpdateEditSession {
    context: ITfContext,
    text: String,
    shared_comp: SharedComposition,
}

impl MazegakiUpdateEditSession {
    pub fn new(
        context: ITfContext,
        text: String,
        shared_comp: SharedComposition,
    ) -> Self {
        MazegakiUpdateEditSession { context, text, shared_comp }
    }
}

impl ITfEditSession_Impl for MazegakiUpdateEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        crate::caret_rect::update_caret_rect(ec, &self.context);
        unsafe {
            if let Some(composition) = self.shared_comp.get() {
                let range = composition.GetRange()?;
                let text_w: Vec<u16> = self.text.encode_utf16().collect();
                range.SetText(ec, TF_ST_CORRECTION, &text_w)?;

                set_cursor_to_end(&self.context, ec, &range)?;

                crate::debug_log!("MazegakiUpdate: updated to '{}'", self.text);
            }
        }
        Ok(())
    }
}

