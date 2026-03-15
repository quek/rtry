//! ITfEditSession 実装
//!
//! TSFではテキストの変更は EditSession 内でのみ許可される。
//! RequestEditSession() で登録し、TSFマネージャが DoEditSession() を呼び出す。
//!
//! SharedComposition (Arc<Mutex<>>) を通じて、EditSession内から
//! TryCodeTextServiceのcomposition状態を更新する。

use windows::core::*;
use windows::Win32::UI::TextServices::*;

use crate::composition::SharedComposition;

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

