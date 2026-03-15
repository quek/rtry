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

        let engine = self.engine.borrow();
        let Some(ref engine) = *engine else {
            return Ok(FALSE);
        };

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
            let mut is_open = self.is_open.borrow_mut();
            let was_open = *is_open;
            *is_open = !was_open;
            crate::debug_log!("IME toggled: {} -> {}", was_open, !was_open);
            drop(is_open);
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

        let mut engine_ref = self.engine.borrow_mut();
        let Some(ref mut engine) = *engine_ref else {
            return Ok(FALSE);
        };

        let Some(ch) = vk_to_char(wparam) else {
            return Ok(FALSE);
        };

        let output = engine.process_key(ch);
        crate::debug_log!("OnKeyDown: ch='{}' output={:?}", ch, output);
        drop(engine_ref);

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
        let engine_ref = self.engine.borrow();
        let Some(ref engine) = *engine_ref else {
            return Ok(());
        };
        let table = engine.table();
        drop(engine_ref);

        let session = edit_session::CharHelpEditSession::new(
            context.clone(),
            table,
        );
        let session: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session, TF_ES_ASYNCDONTCARE | TF_ES_READ)?;
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
}
