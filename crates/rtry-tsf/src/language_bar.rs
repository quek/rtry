//! 言語バー統合
//!
//! タスクバーのIMEインジケーターにtry-codeのモード表示ボタンを追加する。

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::CLSID_TRY_CODE_IME;

/// 言語バーボタンのGUID
const GUID_LANGBAR_ITEM: GUID = GUID::from_u128(0xd9e8f1a2_5b6c_7d8e_9fab_3c4d5e6f7a8b);

/// 言語バーボタン
#[implement(ITfLangBarItemButton, ITfLangBarItem, ITfSource)]
pub struct LangBarButton {
    tooltip: BSTR,
    description: [u16; 32],
}

impl LangBarButton {
    pub fn new() -> Self {
        let desc_str = "try-code";
        let mut description = [0u16; 32];
        for (i, c) in desc_str.encode_utf16().enumerate() {
            if i >= 31 { break; }
            description[i] = c;
        }

        LangBarButton {
            tooltip: BSTR::from("try-code 入力"),
            description,
        }
    }
}

impl ITfLangBarItem_Impl for LangBarButton_Impl {
    fn GetInfo(&self, pinfo: *mut TF_LANGBARITEMINFO) -> Result<()> {
        unsafe {
            let info = &mut *pinfo;
            info.clsidService = CLSID_TRY_CODE_IME;
            info.guidItem = GUID_LANGBAR_ITEM;
            info.dwStyle = TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY;
            info.ulSort = 0;
            info.szDescription = self.description;
        }
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
        // 0 = enabled
        Ok(0)
    }

    fn Show(&self, _fshow: BOOL) -> Result<()> {
        Ok(())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(self.tooltip.clone())
    }
}

impl ITfLangBarItemButton_Impl for LangBarButton_Impl {
    fn OnClick(
        &self,
        _click: TfLBIClick,
        _pt: &POINT,
        _prcarea: *const RECT,
    ) -> Result<()> {
        // TODO: クリック時にモード切替
        Ok(())
    }

    fn InitMenu(&self, _pmenu: Ref<'_, ITfMenu>) -> Result<()> {
        Ok(())
    }

    fn OnMenuSelect(&self, _wid: u32) -> Result<()> {
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        // アイコンなし（テキストのみ表示）
        Ok(HICON::default())
    }

    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::from("漢"))
    }
}

impl ITfSource_Impl for LangBarButton_Impl {
    fn AdviseSink(
        &self,
        _riid: *const GUID,
        _punk: Ref<'_, IUnknown>,
    ) -> Result<u32> {
        // TODO: ITfLangBarItemSink の通知管理
        Ok(1) // cookie
    }

    fn UnadviseSink(&self, _dwcookie: u32) -> Result<()> {
        Ok(())
    }
}

/// 言語バーにボタンを追加
pub fn add_langbar_button(
    thread_mgr: &ITfThreadMgr,
) -> Result<ITfLangBarItemButton> {
    unsafe {
        let langbar_mgr: ITfLangBarItemMgr = thread_mgr.cast()?;
        let button = LangBarButton::new();
        let button_itf: ITfLangBarItemButton = button.into();
        let item: ITfLangBarItem = button_itf.cast()?;
        langbar_mgr.AddItem(&item)?;
        Ok(button_itf)
    }
}

/// 言語バーからボタンを削除
pub fn remove_langbar_button(
    thread_mgr: &ITfThreadMgr,
    button: &ITfLangBarItemButton,
) -> Result<()> {
    unsafe {
        let langbar_mgr: ITfLangBarItemMgr = thread_mgr.cast()?;
        let item: ITfLangBarItem = button.cast()?;
        langbar_mgr.RemoveItem(&item)?;
    }
    Ok(())
}
