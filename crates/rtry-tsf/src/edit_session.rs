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
use crate::text_service::MazegakiState;

/// 文字列確定用のエディットセッション
#[implement(ITfEditSession)]
pub struct CommitEditSession {
    context: ITfContext,
    text: String,
    _client_id: u32,
    shared_comp: SharedComposition,
    composition_sink: ITfCompositionSink,
}

impl CommitEditSession {
    pub fn new(
        context: ITfContext,
        text: String,
        client_id: u32,
        shared_comp: SharedComposition,
        composition_sink: ITfCompositionSink,
    ) -> Self {
        CommitEditSession {
            context, text, _client_id: client_id, shared_comp, composition_sink,
        }
    }
}

impl ITfEditSession_Impl for CommitEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
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
            let sel_range = range.Clone()?;
            sel_range.Collapse(ec, TF_ANCHOR_END)?;
            let selection = TF_SELECTION {
                range: std::mem::ManuallyDrop::new(Some(sel_range)),
                style: TF_SELECTIONSTYLE {
                    ase: TF_AE_NONE,
                    fInterimChar: false.into(),
                },
            };
            let _ = self.context.SetSelection(ec, &[selection]);
            let _ = composition.EndComposition(ec);
        }
        crate::debug_log!("CommitEditSession: committed '{}'", self.text);
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
        unsafe {
            if let Some(composition) = self.shared_comp.get() {
                // 既存のコンポジションを更新
                let range = composition.GetRange()?;
                let text_w: Vec<u16> = self.text.encode_utf16().collect();
                range.SetText(ec, TF_ST_CORRECTION, &text_w)?;
                log::debug!("ComposingEditSession: updated to '{}'", self.text);
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
                let sel_range = comp_range.Clone()?;
                sel_range.Collapse(ec, TF_ANCHOR_END)?;
                let selection = TF_SELECTION {
                    range: std::mem::ManuallyDrop::new(Some(sel_range)),
                    style: TF_SELECTIONSTYLE {
                        ase: TF_AE_NONE,
                        fInterimChar: false.into(),
                    },
                };
                let _ = self.context.SetSelection(ec, &[selection]);

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
    _context: ITfContext,
    _client_id: u32,
    shared_comp: SharedComposition,
}

impl EndCompositionEditSession {
    pub fn new(
        context: ITfContext,
        client_id: u32,
        shared_comp: SharedComposition,
    ) -> Self {
        EndCompositionEditSession { _context: context, _client_id: client_id, shared_comp }
    }
}

impl ITfEditSession_Impl for EndCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            if let Some(composition) = self.shared_comp.take() {
                let range = composition.GetRange()?;
                range.SetText(ec, TF_ST_CORRECTION, &[])?;
                let _ = composition.EndComposition(ec);
                log::debug!("EndCompositionEditSession: ended composition");
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
}

impl CharHelpEditSession {
    pub fn new(context: ITfContext, table: Arc<TryCodeTable>) -> Self {
        CharHelpEditSession { context, table }
    }
}

impl ITfEditSession_Impl for CharHelpEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            // カーソル位置を取得
            let mut sel = [TF_SELECTION::default()];
            let mut fetched = 0u32;
            self.context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut sel, &mut fetched)?;
            if fetched == 0 {
                return Ok(());
            }

            let range = std::mem::ManuallyDrop::into_inner(sel[0].range.clone())
                .ok_or_else(|| Error::from_hresult(E_FAIL))?;

            // カーソル位置を起点にレンジを作成（1文字前に拡張）
            let read_range = range.Clone()?;
            read_range.Collapse(ec, TF_ANCHOR_START)?;
            let mut actual = 0i32;
            read_range.ShiftStart(ec, -1, &mut actual, std::ptr::null())?;

            // テキストを読み取り
            let mut buf = [0u16; 4]; // UTF-16 で最大2ユニット + 余裕
            let mut cch = 0u32;
            read_range.GetText(ec, 0, &mut buf, &mut cch)?;

            if cch == 0 {
                return Ok(());
            }

            let text = String::from_utf16_lossy(&buf[..cch as usize]);
            let ch = text.trim();
            if ch.is_empty() {
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
}

impl MazegakiStartEditSession {
    pub(crate) fn new(
        context: ITfContext,
        shared_comp: SharedComposition,
        composition_sink: ITfCompositionSink,
        dict: Arc<MazegakiDictionary>,
        result_slot: Arc<Mutex<Option<MazegakiState>>>,
    ) -> Self {
        MazegakiStartEditSession {
            context, shared_comp, composition_sink, dict, result_slot,
        }
    }
}

impl ITfEditSession_Impl for MazegakiStartEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            // カーソル位置を取得
            let mut sel = [TF_SELECTION::default()];
            let mut fetched = 0u32;
            self.context.GetSelection(ec, TF_DEFAULT_SELECTION, &mut sel, &mut fetched)?;
            if fetched == 0 {
                return Ok(());
            }

            let range = std::mem::ManuallyDrop::into_inner(sel[0].range.clone())
                .ok_or_else(|| Error::from_hresult(E_FAIL))?;

            // カーソル前の最大10文字を読み取り
            let read_range = range.Clone()?;
            read_range.Collapse(ec, TF_ANCHOR_START)?;
            let mut shifted = 0i32;
            read_range.ShiftStart(ec, -10, &mut shifted, std::ptr::null())?;

            let mut buf = [0u16; 20];
            let mut cch = 0u32;
            read_range.GetText(ec, 0, &mut buf, &mut cch)?;
            if cch == 0 {
                return Ok(());
            }

            let text = String::from_utf16_lossy(&buf[..cch as usize]);
            crate::debug_log!("MazegakiStart: text before cursor = '{}'", text);

            // 最長一致検索
            let Some((reading_len, reading, candidates)) = self.dict.find_longest_match(&text) else {
                crate::debug_log!("MazegakiStart: no match found");
                return Ok(());
            };
            crate::debug_log!("MazegakiStart: matched '{}' ({} chars), {} candidates",
                reading, reading_len, candidates.len());

            // 読みの範囲にコンポジションを開始
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

            // カーソルをコンポジション末尾に移動
            let sel_range = comp_range2.Clone()?;
            sel_range.Collapse(ec, TF_ANCHOR_END)?;
            let selection = TF_SELECTION {
                range: std::mem::ManuallyDrop::new(Some(sel_range)),
                style: TF_SELECTIONSTYLE {
                    ase: TF_AE_NONE,
                    fInterimChar: false.into(),
                },
            };
            let _ = self.context.SetSelection(ec, &[selection]);

            // SharedComposition にセット
            self.shared_comp.set(composition);

            // MazegakiState を結果スロットにセット
            let state = MazegakiState {
                reading,
                candidates,
                selected: 0,
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
        unsafe {
            if let Some(composition) = self.shared_comp.get() {
                let range = composition.GetRange()?;
                let text_w: Vec<u16> = self.text.encode_utf16().collect();
                range.SetText(ec, TF_ST_CORRECTION, &text_w)?;

                let sel_range = range.Clone()?;
                sel_range.Collapse(ec, TF_ANCHOR_END)?;
                let selection = TF_SELECTION {
                    range: std::mem::ManuallyDrop::new(Some(sel_range)),
                    style: TF_SELECTIONSTYLE {
                        ase: TF_AE_NONE,
                        fInterimChar: false.into(),
                    },
                };
                let _ = self.context.SetSelection(ec, &[selection]);

                crate::debug_log!("MazegakiUpdate: updated to '{}'", self.text);
            }
        }
        Ok(())
    }
}

