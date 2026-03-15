//! ITfKeyEventSink 実装
//!
//! キーイベントをrtry-coreのEngineに渡し、結果に応じてEditSessionを起動する。

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;

use rtry_core::engine::EngineOutput;
use rtry_core::table::SpecialFunction;

use crate::edit_session;
use crate::text_service::TryCodeTextService_Impl;

/// Alt キーが押されているか判定
fn is_alt_pressed() -> bool {
    unsafe { GetKeyState(VK_MENU.0 as i32) < 0 }
}

/// IMEオン/オフトグルキーか判定 (Alt+` または 半角/全角)
fn is_toggle_key(wparam: WPARAM) -> bool {
    let vk = wparam.0 as u32;
    (vk == VK_OEM_3.0 as u32 && is_alt_pressed()) || vk == VK_KANJI.0 as u32
}

/// 仮想キーコード (WPARAM) を文字に変換
fn vk_to_char(wparam: WPARAM) -> Option<char> {
    let vk = wparam.0 as u32;
    match vk {
        0x20 => Some(' '),
        0x30..=0x39 => Some((b'0' + (vk - 0x30) as u8) as char),
        0x41..=0x5A => Some((b'a' + (vk - 0x41) as u8) as char),
        // VK_OEM_1 = ';' (0xBA)
        // VK_OEM_MINUS = '-' (0xBD) → ';' として扱う（Pの下が-のキーボード対応）
        0xBA | 0xBD => Some(';'),
        0xBC => Some(','),
        0xBE => Some('.'),
        0xBF => Some('/'),
        _ => None,
    }
}

impl ITfKeyEventSink_Impl for TryCodeTextService_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        _pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        // トグルキーは常に消費
        if is_toggle_key(wparam) {
            return Ok(TRUE);
        }

        // IMEオフならパススルー
        if !*self.is_open.borrow() {
            return Ok(FALSE);
        }

        // 交ぜ書き変換中は全キー消費
        if self.mazegaki_state.lock().unwrap().is_some() {
            return Ok(TRUE);
        }

        let engine = self.engine.borrow();
        let Some(ref engine) = *engine else {
            return Ok(FALSE);
        };

        // ペンディング中の非マップキー処理
        if engine.has_pending_stroke() {
            let vk = wparam.0 as u32;
            if vk == VK_BACK.0 as u32 {
                // BS: 消費してペンディングを破棄
                return Ok(TRUE);
            }
            if vk_to_char(wparam).is_none() {
                // 非マップキー（矢印、Enter等）: パススルー（OnKeyDownでリセット）
                return Ok(FALSE);
            }
        }

        let Some(ch) = vk_to_char(wparam) else {
            return Ok(FALSE);
        };

        if engine.will_consume_key(ch) {
            Ok(TRUE)
        } else {
            Ok(FALSE)
        }
    }

    fn OnTestKeyUp(
        &self,
        _pic: Ref<'_, ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn OnKeyDown(
        &self,
        pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        // IMEオン/オフトグル
        if is_toggle_key(wparam) {
            let was_open = {
                let mut is_open = self.is_open.borrow_mut();
                let was = *is_open;
                *is_open = !was;
                was
            };
            crate::debug_log!("IME toggled: {} -> {}", was_open, !was_open);
            // オフにするときはエンジンとコンポジションをリセット
            if was_open {
                if let Some(ref mut engine) = *self.engine.borrow_mut() {
                    engine.reset();
                }
            }
            return Ok(TRUE);
        }

        // IMEオフならパススルー
        if !*self.is_open.borrow() {
            return Ok(FALSE);
        }

        let context = pic.clone().ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;

        // 交ぜ書き変換中のキー処理
        if self.mazegaki_state.lock().unwrap().is_some() {
            return self.handle_mazegaki_key(&context, wparam);
        }

        let output = {
            let mut engine_ref = self.engine.borrow_mut();
            let Some(ref mut engine) = *engine_ref else {
                return Ok(FALSE);
            };

            // ペンディング中の非マップキー処理
            if engine.has_pending_stroke() {
                let vk = wparam.0 as u32;
                if vk == VK_BACK.0 as u32 {
                    // BS: ペンディングを破棄（消費）
                    crate::debug_log!("OnKeyDown: BS cancels pending stroke");
                    engine.reset();
                    return Ok(TRUE);
                }
                if vk_to_char(wparam).is_none() {
                    // 非マップキー: リセットしてパススルー
                    crate::debug_log!("OnKeyDown: non-mapped key resets pending stroke");
                    engine.reset();
                    return Ok(FALSE);
                }
            }

            let Some(ch) = vk_to_char(wparam) else {
                return Ok(FALSE);
            };

            let output = engine.process_key(ch);
            crate::debug_log!("OnKeyDown: ch='{}' output={:?}", ch, output);
            output
        };

        match output {
            EngineOutput::Commit(text) => {
                self.do_commit(&context, &text)?;
                Ok(TRUE)
            }
            EngineOutput::Composing(text) => {
                self.do_composing(&context, &text)?;
                Ok(TRUE)
            }
            EngineOutput::Clear => {
                self.do_end_composition(&context)?;
                Ok(TRUE)
            }
            EngineOutput::Consumed => Ok(TRUE),
            EngineOutput::PassThrough => Ok(FALSE),
            EngineOutput::SpecialAction(func) => {
                match func {
                    SpecialFunction::CharHelp(_) => {
                        self.do_char_help(&context)?;
                    }
                    SpecialFunction::MazegakiConvert => {
                        self.do_mazegaki_start(&context)?;
                    }
                    _ => {
                        crate::debug_log!("Unhandled special action: {:?}", func);
                    }
                }
                Ok(TRUE)
            }
        }
    }

    fn OnKeyUp(
        &self,
        _pic: Ref<'_, ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn OnPreservedKey(
        &self,
        _pic: Ref<'_, ITfContext>,
        _rguid: *const GUID,
    ) -> Result<BOOL> {
        Ok(FALSE)
    }
}

impl TryCodeTextService_Impl {
    /// テキストを確定出力する
    fn do_commit(&self, context: &ITfContext, text: &str) -> Result<()> {
        let tid = *self.client_id.borrow();
        let this_unknown: IUnknown = self.to_interface();
        let comp_sink: ITfCompositionSink = this_unknown.cast()?;
        let session = edit_session::CommitEditSession::new(
            context.clone(),
            text.to_string(),
            tid,
            self.composition.clone(),
            comp_sink,
        );
        let session: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE)?;
        }
        Ok(())
    }

    /// 合成文字列を表示する
    fn do_composing(&self, context: &ITfContext, text: &str) -> Result<()> {
        let tid = *self.client_id.borrow();

        let this_unknown: IUnknown = self.to_interface();
        let comp_sink: ITfCompositionSink = this_unknown.cast()?;

        let session = edit_session::ComposingEditSession::new(
            context.clone(),
            text.to_string(),
            tid,
            self.composition.clone(),
            comp_sink,
        );
        let session: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE)?;
        }
        Ok(())
    }

    /// ストロークヘルプを表示する
    fn do_char_help(&self, context: &ITfContext) -> Result<()> {
        let tid = *self.client_id.borrow();
        let table = {
            let engine_ref = self.engine.borrow();
            let Some(ref engine) = *engine_ref else {
                return Ok(());
            };
            engine.table()
        };

        let session = edit_session::CharHelpEditSession::new(
            context.clone(),
            table,
        );
        let session: ITfEditSession = session.into();
        unsafe {
            let hr = context.RequestEditSession(tid, &session, TF_ES_ASYNCDONTCARE | TF_ES_READ)?;
            crate::debug_log!("CharHelp: RequestEditSession returned hr={:?}", hr);
        }
        Ok(())
    }

    /// コンポジションを終了する
    fn do_end_composition(&self, context: &ITfContext) -> Result<()> {
        let tid = *self.client_id.borrow();
        let session = edit_session::EndCompositionEditSession::new(
            context.clone(),
            tid,
            self.composition.clone(),
        );
        let session: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE)?;
        }
        Ok(())
    }

    /// 交ぜ書き変換を開始する
    fn do_mazegaki_start(&self, context: &ITfContext) -> Result<()> {
        let dict = {
            let dict_ref = self.mazegaki_dict.borrow();
            let Some(ref dict) = *dict_ref else {
                crate::debug_log!("Mazegaki: no dictionary loaded");
                return Ok(());
            };
            dict.clone()
        };

        let tid = *self.client_id.borrow();
        let this_unknown: IUnknown = self.to_interface();
        let comp_sink: ITfCompositionSink = this_unknown.cast()?;

        let session = edit_session::MazegakiStartEditSession::new(
            context.clone(),
            self.composition.clone(),
            comp_sink,
            dict,
            self.mazegaki_state.clone(),
        );
        let session: ITfEditSession = session.into();
        unsafe {
            let hr = context.RequestEditSession(tid, &session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE)?;
            crate::debug_log!("MazegakiStart: RequestEditSession returned hr={:?}", hr);
        }

        // EditSession 完了後、候補ウィンドウを表示
        let state = self.mazegaki_state.lock().unwrap();
        if let Some(ref state) = *state {
            crate::candidate_window::show_candidates(&state.candidates, state.selected);
        }

        Ok(())
    }

    /// 交ぜ書き変換中のキー処理
    fn handle_mazegaki_key(&self, context: &ITfContext, wparam: WPARAM) -> Result<BOOL> {
        let vk = wparam.0 as u32;

        match vk {
            // Space: 次候補
            0x20 => {
                let (text, selected, candidates) = {
                    let mut guard = self.mazegaki_state.lock().unwrap();
                    let Some(ref mut state) = *guard else {
                        return Ok(TRUE);
                    };
                    state.selected = (state.selected + 1) % state.candidates.len();
                    (
                        state.candidates[state.selected].clone(),
                        state.selected,
                        state.candidates.clone(),
                    )
                };
                self.do_mazegaki_update(context, &text)?;
                crate::candidate_window::show_candidates(&candidates, selected);
                Ok(TRUE)
            }
            // Enter: 確定
            0x0D => {
                self.do_mazegaki_commit(context)?;
                Ok(TRUE)
            }
            // Escape: キャンセル
            0x1B => {
                self.do_mazegaki_cancel(context)?;
                Ok(TRUE)
            }
            // 1-9: 番号選択で確定
            0x31..=0x39 => {
                let index = (vk - 0x31) as usize;
                {
                    let mut guard = self.mazegaki_state.lock().unwrap();
                    if let Some(ref mut state) = *guard {
                        if index < state.candidates.len() {
                            state.selected = index;
                        }
                    }
                }
                self.do_mazegaki_commit(context)?;
                Ok(TRUE)
            }
            // その他: 現在の候補で確定して、そのキーを通常処理へ
            _ => {
                self.do_mazegaki_commit(context)?;
                // 確定後に通常のキー処理を行う
                Ok(FALSE)
            }
        }
    }

    /// 交ぜ書き候補のテキストを更新
    fn do_mazegaki_update(&self, context: &ITfContext, text: &str) -> Result<()> {
        let tid = *self.client_id.borrow();
        let session = edit_session::MazegakiUpdateEditSession::new(
            context.clone(),
            text.to_string(),
            self.composition.clone(),
        );
        let session: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE)?;
        }
        Ok(())
    }

    /// 交ぜ書き変換を確定
    fn do_mazegaki_commit(&self, context: &ITfContext) -> Result<()> {
        let state = self.mazegaki_state.lock().unwrap().take();
        crate::candidate_window::dismiss();

        if let Some(state) = state {
            let text = state.candidates[state.selected].clone();
            self.do_commit(context, &text)?;
            self.show_mazegaki_stroke_help(&text);
        }
        Ok(())
    }

    /// 交ぜ書き確定後にストロークヘルプを表示
    fn show_mazegaki_stroke_help(&self, text: &str) {
        let table = {
            let engine_ref = self.engine.borrow();
            let Some(ref engine) = *engine_ref else {
                return;
            };
            engine.table()
        };

        let mut parts = Vec::new();
        for ch in text.chars() {
            let s = ch.to_string();
            let strokes = table.reverse_lookup(&s);
            if strokes.is_empty() {
                parts.push(format!("{}:?", ch));
            } else {
                let stroke_strs: Vec<String> = strokes.iter()
                    .map(|s| s.to_display_string())
                    .collect();
                parts.push(format!("{}:{}", ch, stroke_strs.join("/")));
            }
        }
        if !parts.is_empty() {
            let msg = parts.join("  ");
            crate::stroke_help::show_stroke_help(&msg);
        }
    }

    /// 交ぜ書き変換をキャンセル（元の読みに戻す）
    fn do_mazegaki_cancel(&self, context: &ITfContext) -> Result<()> {
        let state = self.mazegaki_state.lock().unwrap().take();
        crate::candidate_window::dismiss();

        if let Some(state) = state {
            // 元の読みで確定（元に戻す）
            self.do_commit(context, &state.reading)?;
        }
        Ok(())
    }
}
