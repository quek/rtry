//! 言語バー統合
//!
//! タスクバーのIMEインジケーターにtry-codeのモード表示ボタンを追加する。
//! CorvusSKK パターン: 2つのボタンを登録する。
//!   1. 独自GUID ボタン（TF_LBI_STYLE_BTN_MENU） — 従来の言語バー用
//!   2. GUID_LBI_INPUTMODE ボタン（TF_LBI_STYLE_BTN_BUTTON） — Windows 8+ タスクバー統合用

use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::CLSID_TRY_CODE_IME;

/// 言語バーボタンのGUID（従来の言語バー用）
const GUID_LANGBAR_ITEM: GUID = GUID::from_u128(0xd9e8f1a2_5b6c_7d8e_9fab_3c4d5e6f7a8b);

/// メニュー項目ID
const MENU_ID_CONFIG: u32 = 1;

/// 言語バーボタン
#[implement(ITfLangBarItemButton, ITfLangBarItem, ITfSource)]
pub struct LangBarButton {
    guid: GUID,
    is_open: Arc<AtomicBool>,
    sink: Mutex<Option<ITfLangBarItemSink>>,
    tooltip: BSTR,
    description: [u16; 32],
}

impl LangBarButton {
    pub fn new(guid: GUID, is_open: Arc<AtomicBool>) -> Self {
        let desc_str = "try-code";
        let mut description = [0u16; 32];
        for (i, c) in desc_str.encode_utf16().enumerate() {
            if i >= 31 { break; }
            description[i] = c;
        }

        LangBarButton {
            guid,
            is_open,
            sink: Mutex::new(None),
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
            info.guidItem = self.guid;
            info.dwStyle = if self.guid == GUID_LBI_INPUTMODE {
                TF_LBI_STYLE_BTN_BUTTON
            } else {
                TF_LBI_STYLE_BTN_MENU | TF_LBI_STYLE_SHOWNINTRAY
            };
            info.ulSort = 0;
            info.szDescription = self.description;
        }
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
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
        click: TfLBIClick,
        pt: &POINT,
        _prcarea: *const RECT,
    ) -> Result<()> {
        if self.guid == GUID_LBI_INPUTMODE && click == TF_LBI_CLK_RIGHT {
            show_context_menu(pt);
        }
        Ok(())
    }

    fn InitMenu(&self, pmenu: Ref<'_, ITfMenu>) -> Result<()> {
        let menu = (*pmenu).as_ref().ok_or(E_INVALIDARG)?;
        let text: Vec<u16> = "設定...".encode_utf16().collect();
        unsafe {
            menu.AddMenuItem(
                MENU_ID_CONFIG,
                0,
                HBITMAP::default(),
                HBITMAP::default(),
                &text,
                std::ptr::null_mut(),
            )?;
        }
        Ok(())
    }

    fn OnMenuSelect(&self, wid: u32) -> Result<()> {
        if wid == MENU_ID_CONFIG {
            launch_config();
        }
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        if self.is_open.load(Ordering::Relaxed) {
            Ok(create_text_icon("漢"))
        } else {
            Ok(create_text_icon("A"))
        }
    }

    fn GetText(&self) -> Result<BSTR> {
        if self.is_open.load(Ordering::Relaxed) {
            Ok(BSTR::from("漢"))
        } else {
            Ok(BSTR::from("A"))
        }
    }
}

impl ITfSource_Impl for LangBarButton_Impl {
    fn AdviseSink(
        &self,
        riid: *const GUID,
        punk: Ref<'_, IUnknown>,
    ) -> Result<u32> {
        let iid = unsafe { &*riid };
        if *iid == ITfLangBarItemSink::IID {
            if let Some(unknown) = (*punk).as_ref() {
                if let Ok(sink) = unknown.cast::<ITfLangBarItemSink>() {
                    *self.sink.lock().unwrap() = Some(sink);
                }
            }
        }
        Ok(1)
    }

    fn UnadviseSink(&self, _dwcookie: u32) -> Result<()> {
        *self.sink.lock().unwrap() = None;
        Ok(())
    }
}

/// 言語バーにボタンを追加（独自GUID + GUID_LBI_INPUTMODE の2つ）
pub fn add_langbar_buttons(
    thread_mgr: &ITfThreadMgr,
    is_open: Arc<AtomicBool>,
) -> Result<(ITfLangBarItemButton, ITfLangBarItemButton)> {
    unsafe {
        let langbar_mgr: ITfLangBarItemMgr = thread_mgr.cast()?;

        // 1. 従来の言語バー用ボタン（BTN_MENU）
        let button = LangBarButton::new(GUID_LANGBAR_ITEM, is_open.clone());
        let button_itf: ITfLangBarItemButton = button.into();
        let item: ITfLangBarItem = button_itf.cast()?;
        langbar_mgr.AddItem(&item)?;

        // 2. Windows 8+ タスクバーIMEインジケーター統合用（BTN_BUTTON）
        let input_mode = LangBarButton::new(GUID_LBI_INPUTMODE, is_open);
        let input_mode_itf: ITfLangBarItemButton = input_mode.into();
        let item_i: ITfLangBarItem = input_mode_itf.cast()?;
        langbar_mgr.AddItem(&item_i)?;

        Ok((button_itf, input_mode_itf))
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

/// 右クリック時にポップアップメニューを表示
fn show_context_menu(pt: &POINT) {
    unsafe {
        let hmenu = CreatePopupMenu();
        let Ok(hmenu) = hmenu else { return };

        let text: Vec<u16> = "設定...\0".encode_utf16().collect();
        let _ = AppendMenuW(hmenu, MF_STRING, MENU_ID_CONFIG as usize, PCWSTR(text.as_ptr()));

        let _ = SetForegroundWindow(GetForegroundWindow());

        let cmd = TrackPopupMenuEx(
            hmenu,
            (TPM_LEFTALIGN | TPM_TOPALIGN | TPM_NONOTIFY | TPM_RETURNCMD).0,
            pt.x,
            pt.y,
            GetForegroundWindow(),
            None,
        );
        let _ = DestroyMenu(hmenu);

        if cmd.0 == MENU_ID_CONFIG as i32 {
            launch_config();
        }
    }
}

/// テキストからアイコンを生成
fn create_text_icon(text: &str) -> HICON {
    unsafe {
        let size = 16;
        let hdc_screen = GetDC(None);
        let hdc = CreateCompatibleDC(Some(hdc_screen));
        let hbmp = CreateCompatibleBitmap(hdc_screen, size, size);
        let old_bmp = SelectObject(hdc, hbmp.into());

        let bg_brush = CreateSolidBrush(COLORREF(0x00804000));
        let rect = RECT { left: 0, top: 0, right: size, bottom: size };
        FillRect(hdc, &rect, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(0x00FFFFFF));

        let font = CreateFontW(
            14, 0, 0, 0, 700,
            0, 0, 0,
            SHIFTJIS_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("MS Gothic"),
        );
        let old_font = SelectObject(hdc, font.into());

        let text_wide: Vec<u16> = text.encode_utf16().collect();
        let _ = DrawTextW(hdc, &mut text_wide.clone(), &mut rect.clone(),
            DT_CENTER | DT_VCENTER | DT_SINGLELINE);

        SelectObject(hdc, old_font);
        let _ = DeleteObject(font.into());
        SelectObject(hdc, old_bmp);

        let hbmp_mask = CreateCompatibleBitmap(hdc_screen, size, size);

        let mut icon_info = ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbmp_mask,
            hbmColor: hbmp,
        };
        let hicon = CreateIconIndirect(&mut icon_info).unwrap_or_default();

        let _ = DeleteObject(hbmp.into());
        let _ = DeleteObject(hbmp_mask.into());
        let _ = DeleteDC(hdc);
        ReleaseDC(None, hdc_screen);

        hicon
    }
}

/// DLLと同じディレクトリの rtry-config.exe を起動
pub fn launch_config() {
    let mut buf = vec![0u16; 260];
    let len = unsafe { GetModuleFileNameW(Some(crate::dll_module()), &mut buf) } as usize;
    if len == 0 {
        return;
    }
    let dll_path = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len]));
    let Some(dir) = dll_path.parent() else { return };
    let config_exe = dir.join("rtry-config.exe");

    let exe_wide: Vec<u16> = config_exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let open_wide: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(open_wide.as_ptr()),
            PCWSTR(exe_wide.as_ptr()),
            None,
            None,
            SW_SHOW,
        );
    }
}
