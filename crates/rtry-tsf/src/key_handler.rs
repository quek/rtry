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

/// Ctrl または Shift が押されているか判定
fn has_modifier_key() -> bool {
    unsafe {
        GetKeyState(VK_CONTROL.0 as i32) < 0
            || GetKeyState(VK_SHIFT.0 as i32) < 0
            || GetKeyState(VK_MENU.0 as i32) < 0
    }
}

/// Shift のみが押されているか判定（Ctrl/Alt なし）
fn is_shift_only() -> bool {
    unsafe {
        GetKeyState(VK_SHIFT.0 as i32) < 0
            && GetKeyState(VK_CONTROL.0 as i32) >= 0
            && GetKeyState(VK_MENU.0 as i32) >= 0
    }
}

/// IMEオン/オフトグルキーか判定 (Alt+` または 半角/全角)
fn is_toggle_key(wparam: WPARAM) -> bool {
    let vk = wparam.0 as u32;
    (vk == VK_OEM_3.0 as u32 && is_alt_pressed()) || vk == VK_KANJI.0 as u32
}

/// 仮想キーコード (WPARAM) を T-Code 40キー配列の文字に変換
fn vk_to_char(wparam: WPARAM) -> Option<char> {
    let vk = wparam.0 as u32;
    match vk {
        0x20 => Some(' '),
        0x30..=0x39 => Some((b'0' + (vk - 0x30) as u8) as char),
        0x41..=0x5A => Some((b'a' + (vk - 0x41) as u8) as char),
        0xBA => Some(';'), // VK_OEM_1
        0xBD => Some('-'), // VK_OEM_MINUS
        0xBC => Some(','),
        0xBE => Some('.'),
        0xBF => Some('/'),
        _ => None,
    }
}

/// 仮想キーコードを文字に変換（MapVirtualKeyW によるフォールバック）
/// T-Code 40キー配列外のキーをプレフィックスキーとして使う場合に使用
fn vk_to_any_char(wparam: WPARAM) -> Option<char> {
    vk_to_char(wparam).or_else(|| {
        let vk = wparam.0 as u32;
        let mapped = unsafe { MapVirtualKeyW(vk, MAP_VIRTUAL_KEY_TYPE(2)) };
        if mapped > 0 {
            char::from_u32(mapped)
        } else {
            None
        }
    })
}

/// キーの押下・解放の INPUT ペアを生成する
fn make_key_input_pair(vk: VIRTUAL_KEY) -> [INPUT; 2] {
    let ki = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    [ki(Default::default()), ki(KEYEVENTF_KEYUP)]
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
        if !self.is_open.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(FALSE);
        }

        // 交ぜ書き変換中は Ctrl/Alt 付きのみパススルー（Shift-Space 等は消費）
        if self.mazegaki_state.lock().unwrap().is_some() {
            if has_modifier_key() && !is_shift_only() {
                return Ok(FALSE);
            }
            return Ok(TRUE);
        }

        // 修飾キー（Ctrl/Shift/Alt）付きはパススルー（C-c, C-v 等）
        if has_modifier_key() {
            return Ok(FALSE);
        }

        // CUAS遅延置換中の VK_BACK 処理
        if wparam.0 as u32 == VK_BACK.0 as u32 {
            let mut pending = self.pending_replace.borrow_mut();
            if let Some(ref mut p) = *pending {
                if p.remaining_bs > 0 {
                    // 読み削除用: アプリに渡す
                    p.remaining_bs -= 1;
                    return Ok(FALSE);
                } else {
                    // 番兵: IMEが消費して置換テキストを確定
                    return Ok(TRUE);
                }
            }
        }

        let engine = self.engine.borrow();
        let Some(ref engine) = *engine else {
            return Ok(FALSE);
        };

        let prefix_key = engine.ext_prefix_key();

        // ペンディング中の非マップキー処理
        if engine.has_pending_stroke() {
            let vk = wparam.0 as u32;
            if vk == VK_BACK.0 as u32 {
                // BS: 消費してペンディングを破棄
                return Ok(TRUE);
            }
            // T-Code キーでもプレフィックスキーでもなければパススルー
            if vk_to_char(wparam).is_none()
                && vk_to_any_char(wparam) != Some(prefix_key)
            {
                // 非マップキー（矢印、Enter等）: パススルー（OnKeyDownでリセット）
                return Ok(FALSE);
            }
        }

        // BS（エンジンIdle時）: postbufから1文字削除してアプリにパススルー
        if wparam.0 as u32 == VK_BACK.0 as u32 {
            crate::text_service::postbuf_remove_tail(&self.postbuf, 1);
            return Ok(FALSE);
        }

        // T-Code 40キーまたはプレフィックスキーに変換
        let ch = vk_to_char(wparam).or_else(|| {
            let c = vk_to_any_char(wparam)?;
            (c == prefix_key).then_some(c)
        });
        let Some(ch) = ch else {
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
            let was_open = self.is_open.load(std::sync::atomic::Ordering::Relaxed);
            self.is_open.store(!was_open, std::sync::atomic::Ordering::Relaxed);
            crate::debug_log!("IME toggled: {} -> {}", was_open, !was_open);
            // 言語バーアイコンを更新
            self.notify_langbar_update();
            // オフにするときはエンジンとコンポジションをリセット
            if was_open {
                if let Some(ref mut engine) = *self.engine.borrow_mut() {
                    engine.reset();
                }
                self.postbuf.lock().unwrap().clear();
                crate::ime_indicator::dismiss();
            } else {
                crate::ime_indicator::show();
                // EditSession 経由で GetTextExt → インジケーター表示
                if let Some(context) = pic.clone() {
                    let _ = self.do_show_indicator(&context);
                }
            }
            return Ok(TRUE);
        }

        // IMEオフならパススルー
        if !self.is_open.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(FALSE);
        }

        crate::ime_indicator::update_position();

        let context = pic.clone().ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;

        // 交ぜ書き変換中のキー処理（Shift-Space 等のため修飾キーチェックより先）
        if self.mazegaki_state.lock().unwrap().is_some() {
            // Ctrl/Alt 付きはパススルー（C-c, C-v 等）
            if has_modifier_key() && !is_shift_only() {
                return Ok(FALSE);
            }
            return self.handle_mazegaki_key(&context, wparam);
        }

        // 修飾キー（Ctrl/Shift/Alt）付きはパススルー（C-c, C-v 等）
        // 注: 一部のアプリ（Windows 11 メモ帳等）は OnTestKeyDown を呼ばず
        // OnKeyDown を直接呼ぶため、ここでもチェックが必要
        if has_modifier_key() {
            return Ok(FALSE);
        }

        // CUAS遅延置換中の VK_BACK 処理
        if wparam.0 as u32 == VK_BACK.0 as u32 {
            let mut pending = self.pending_replace.borrow_mut();
            if let Some(ref mut p) = *pending {
                if p.remaining_bs > 0 {
                    // 読み削除用: アプリに渡す
                    p.remaining_bs -= 1;
                    return Ok(FALSE);
                } else {
                    // 番兵: IMEが消費して置換テキストを確定
                    let text = p.text.clone();
                    drop(pending);
                    self.pending_replace.borrow_mut().take();
                    crate::debug_log!("PendingReplace: sentinel arrived, committing '{}'", text);
                    self.do_commit(&context, &text)?;
                    return Ok(TRUE);
                }
            }
        }

        let output = {
            let mut engine_ref = self.engine.borrow_mut();
            let Some(ref mut engine) = *engine_ref else {
                return Ok(FALSE);
            };

            let prefix_key = engine.ext_prefix_key();

            // ペンディング中の非マップキー処理
            if engine.has_pending_stroke() {
                let vk = wparam.0 as u32;
                if vk == VK_BACK.0 as u32 {
                    // BS: ペンディングを破棄（消費）
                    crate::debug_log!("OnKeyDown: BS cancels pending stroke");
                    engine.reset();
                    return Ok(TRUE);
                }
                // T-Code キーでもプレフィックスキーでもなければパススルー
                if vk_to_char(wparam).is_none()
                    && vk_to_any_char(wparam) != Some(prefix_key)
                {
                    // 非マップキー: リセットしてパススルー
                    crate::debug_log!("OnKeyDown: non-mapped key resets pending stroke");
                    engine.reset();
                    return Ok(FALSE);
                }
            }

            // BS: アプリにパススルーし、postbuf からも1文字削除
            if wparam.0 as u32 == VK_BACK.0 as u32 {
                crate::text_service::postbuf_remove_tail(&self.postbuf, 1);
                return Ok(FALSE);
            }

            // T-Code 40キーまたはプレフィックスキーに変換
            let ch = vk_to_char(wparam).or_else(|| {
                let c = vk_to_any_char(wparam)?;
                (c == prefix_key).then_some(c)
            });
            let Some(ch) = ch else {
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
                    SpecialFunction::CharHelp(true) => {
                        self.do_char_help(&context)?;
                    }
                    SpecialFunction::CharHelp(false) => {
                        crate::stroke_help::reshow_last_help();
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
            self.postbuf.clone(),
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
            self.postbuf.clone(),
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
            self.postbuf.clone(),
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

        // 修飾キー単体（Shift/Ctrl/Alt）は無視
        if matches!(vk, 0x10 | 0x11 | 0x12 | 0xA0..=0xA5) {
            return Ok(TRUE);
        }

        let page_size = crate::candidate_window::PAGE_SIZE;

        match vk {
            // Space: 次候補 / Shift-Space: 前候補
            // Down: 次候補 / Up: 前候補
            // PgDn: 次ページ / PgUp: 前ページ
            0x20 | 0x26 | 0x28 | 0x22 | 0x21 => {
                let (text, selected, is_postbuf) = {
                    let mut guard = self.mazegaki_state.lock().unwrap();
                    let Some(ref mut state) = *guard else {
                        return Ok(TRUE);
                    };
                    let len = state.candidates.len();
                    state.selected = match vk {
                        // Space: Shift なら前候補、なしなら次候補
                        0x20 => {
                            if is_shift_only() {
                                (state.selected + len - 1) % len
                            } else {
                                (state.selected + 1) % len
                            }
                        }
                        // Down: 次候補
                        0x28 => (state.selected + 1) % len,
                        // Up: 前候補
                        0x26 => (state.selected + len - 1) % len,
                        // PgDn: 次ページ先頭
                        0x22 => {
                            let page = state.selected / page_size;
                            let total_pages = (len + page_size - 1) / page_size;
                            let next_page = if page + 1 < total_pages { page + 1 } else { 0 };
                            (next_page * page_size).min(len - 1)
                        }
                        // PgUp: 前ページ先頭
                        _ => {
                            let page = state.selected / page_size;
                            let total_pages = (len + page_size - 1) / page_size;
                            let prev_page = if page > 0 { page - 1 } else { total_pages - 1 };
                            (prev_page * page_size).min(len - 1)
                        }
                    };
                    (
                        state.candidates[state.selected].clone(),
                        state.selected,
                        state.postbuf_reading_len.is_some(),
                    )
                };
                if !is_postbuf {
                    self.do_mazegaki_update(context, &text)?;
                }
                crate::candidate_window::update_selected(selected);
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
            // Left: 読みを伸ばす / Right: 読みを縮める
            0x25 | 0x27 => {
                self.do_mazegaki_resize(context, vk == 0x25)?;
                Ok(TRUE)
            }
            // 3段目キー(a-;): ページ内のショートカット選択で確定
            _ if {
                let labels = crate::candidate_window::current_labels();
                vk_to_char(wparam).is_some_and(|ch| labels.contains(&ch))
            } => {
                let labels = crate::candidate_window::current_labels();
                let ch = vk_to_char(wparam).unwrap();
                let offset_in_page = labels.iter().position(|&l| l == ch).unwrap();
                {
                    let mut guard = self.mazegaki_state.lock().unwrap();
                    if let Some(ref mut state) = *guard {
                        let page_size = crate::candidate_window::PAGE_SIZE;
                        let page_start = (state.selected / page_size) * page_size;
                        let abs_index = page_start + offset_in_page;
                        if abs_index < state.candidates.len() {
                            state.selected = abs_index;
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

    /// 交ぜ書き変換の読み長さを変更（Left=伸ばし, Right=縮め）
    fn do_mazegaki_resize(&self, context: &ITfContext, extend: bool) -> Result<()> {
        let dict = {
            let dict_ref = self.mazegaki_dict.borrow();
            let Some(ref dict) = *dict_ref else { return Ok(()); };
            dict.clone()
        };

        let (pre_text, current_len, original_reading, is_postbuf) = {
            let guard = self.mazegaki_state.lock().unwrap();
            let Some(ref state) = *guard else { return Ok(()); };
            (
                state.pre_text.clone(),
                state.reading_len,
                state.reading.clone(),
                state.postbuf_reading_len.is_some(),
            )
        };

        // CUAS環境ではリサイズ非対応
        if is_postbuf { return Ok(()); }

        let result = if extend {
            dict.find_longer_match(&pre_text, current_len)
        } else {
            dict.find_shorter_match(&pre_text, current_len)
        };

        let Some((new_reading_len, new_candidates)) = result else {
            crate::debug_log!("MazegakiResize: no {} match from {} chars",
                if extend { "longer" } else { "shorter" }, current_len);
            return Ok(());
        };

        // MazegakiState を更新
        let text_chars: Vec<char> = pre_text.chars().collect();
        let reading_start = text_chars.len() - new_reading_len;
        let new_reading: String = text_chars[reading_start..].iter().collect();
        {
            let mut guard = self.mazegaki_state.lock().unwrap();
            if let Some(ref mut state) = *guard {
                state.reading = new_reading;
                state.reading_len = new_reading_len;
                state.candidates = new_candidates.clone();
                state.selected = 0;
            }
        }

        // EditSession でコンポジション範囲を変更
        let tid = *self.client_id.borrow();
        let this_unknown: IUnknown = self.to_interface();
        let comp_sink: ITfCompositionSink = this_unknown.cast()?;
        let session = edit_session::MazegakiResizeEditSession::new(
            context.clone(),
            self.composition.clone(),
            comp_sink,
            original_reading,
            new_reading_len,
            new_candidates[0].clone(),
        );
        let session: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE)?;
        }

        // 候補ウィンドウ更新
        let state = self.mazegaki_state.lock().unwrap();
        if let Some(ref state) = *state {
            crate::candidate_window::show_candidates(&state.candidates, state.selected);
        }

        Ok(())
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
            if let Some(reading_len) = state.postbuf_reading_len {
                // CUAS環境: VKBackBasedDeleter パターン（tsf-tutcode/Mozc 由来）
                // N+1 個の VK_BACK を SendInput で送信:
                //   最初の N 個 → OnTestKeyDown で FALSE を返しアプリに渡す（読み削除）
                //   最後の 1 個（番兵） → OnTestKeyDown で TRUE → OnKeyDown で do_commit
                // TSFコールバックは同一スレッドで直列処理されるため順序保証される
                crate::text_service::postbuf_remove_tail(&self.postbuf, reading_len);
                *self.pending_replace.borrow_mut() = Some(
                    crate::text_service::PendingReplace {
                        remaining_bs: reading_len,
                        text: text.clone(),
                    },
                );
                self.send_backspaces(reading_len + 1); // +1 = 番兵
                crate::debug_log!(
                    "MazegakiCommit(postbuf): queued {} BS + sentinel for '{}'",
                    reading_len, text
                );
            } else {
                self.do_commit(context, &text)?;
            }
            self.show_mazegaki_stroke_help(&text);
        }
        Ok(())
    }

    /// SendInput で VK_BACK を送信する
    fn send_backspaces(&self, count: usize) {
        let inputs: Vec<INPUT> = (0..count)
            .flat_map(|_| make_key_input_pair(VK_BACK))
            .collect();
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    /// 交ぜ書き確定後にストロークヘルプを表示
    fn show_mazegaki_stroke_help(&self, text: &str) {
        use std::fmt::Write;

        let table = {
            let engine_ref = self.engine.borrow();
            let Some(ref engine) = *engine_ref else {
                return;
            };
            engine.table()
        };

        let mut buf = String::new();
        for ch in text.chars() {
            if !buf.is_empty() {
                buf.push_str("  ");
            }
            let s = ch.to_string();
            let strokes = table.reverse_lookup(&s);
            if strokes.is_empty() {
                let _ = write!(buf, "{}:?", ch);
            } else {
                let _ = write!(buf, "{}:", ch);
                for (i, s) in strokes.iter().enumerate() {
                    if i > 0 {
                        buf.push('/');
                    }
                    buf.push_str(&s.to_display_string(table.key_layout_40()));
                }
            }
        }
        if !buf.is_empty() {
            crate::stroke_help::show_stroke_help(&buf);
        }
    }

    /// IMEインジケーターを表示する（EditSession 経由で位置取得）
    fn do_show_indicator(&self, context: &ITfContext) -> Result<()> {
        let tid = *self.client_id.borrow();
        let session = edit_session::IndicatorEditSession::new(context.clone());
        let session: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session, TF_ES_ASYNCDONTCARE | TF_ES_READ)?;
        }
        Ok(())
    }

    /// 交ぜ書き変換をキャンセル（元の読みに戻す）
    fn do_mazegaki_cancel(&self, context: &ITfContext) -> Result<()> {
        let state = self.mazegaki_state.lock().unwrap().take();
        crate::candidate_window::dismiss();

        if let Some(state) = state {
            if state.postbuf_reading_len.is_some() {
                // CUAS環境: コンポジションを作っていないので何もしない
                crate::debug_log!("MazegakiCancel(postbuf): cancelled, reading unchanged");
            } else {
                // 通常環境: 元の読みで確定（元に戻す）
                self.do_commit(context, &state.reading)?;
            }
        }
        Ok(())
    }
}
