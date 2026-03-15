//! DLL登録/解除 (regsvr32 対応)
//!
//! COM InprocServer32 レジストリ登録 + TSF カテゴリ/プロファイル登録

use windows::core::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::*;

use crate::{CLSID_TRY_CODE_IME, GUID_PROFILE, dll_module};

/// 言語ID: 日本語
const LANGID_JAPANESE: u16 = 0x0411;

/// IMEの表示名
const IME_DISPLAY_NAME: &str = "Try-Code";

/// GUIDを文字列に変換 ("{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}")
fn guid_to_string(guid: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        guid.data1, guid.data2, guid.data3,
        guid.data4[0], guid.data4[1], guid.data4[2], guid.data4[3],
        guid.data4[4], guid.data4[5], guid.data4[6], guid.data4[7],
    )
}

/// COM InprocServer32 レジストリ登録
fn register_com_server(dll_path: &str) -> Result<()> {
    unsafe {
        let clsid_str = guid_to_string(&CLSID_TRY_CODE_IME);

        // CLSID キー作成
        let subkey = format!("CLSID\\{clsid_str}");
        let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hkey = HKEY::default();
        RegCreateKeyW(HKEY_CLASSES_ROOT, PCWSTR(subkey_w.as_ptr()), &mut hkey).ok()?;

        // デフォルト値にIME名を設定
        let name_w: Vec<u16> = IME_DISPLAY_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        RegSetValueExW(
            hkey, None, Some(0), REG_SZ,
            Some(std::slice::from_raw_parts(name_w.as_ptr() as *const u8, name_w.len() * 2)),
        ).ok()?;
        let _ = RegCloseKey(hkey);

        // InprocServer32 サブキー
        let inproc_subkey = format!("CLSID\\{clsid_str}\\InprocServer32");
        let inproc_w: Vec<u16> = inproc_subkey.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hkey_inproc = HKEY::default();
        RegCreateKeyW(HKEY_CLASSES_ROOT, PCWSTR(inproc_w.as_ptr()), &mut hkey_inproc).ok()?;

        // デフォルト値にDLLパスを設定
        let path_w: Vec<u16> = dll_path.encode_utf16().chain(std::iter::once(0)).collect();
        RegSetValueExW(
            hkey_inproc, None, Some(0), REG_SZ,
            Some(std::slice::from_raw_parts(path_w.as_ptr() as *const u8, path_w.len() * 2)),
        ).ok()?;

        // ThreadingModel = "Apartment"
        let threading_name: Vec<u16> = "ThreadingModel".encode_utf16().chain(std::iter::once(0)).collect();
        let threading_val: Vec<u16> = "Apartment".encode_utf16().chain(std::iter::once(0)).collect();
        RegSetValueExW(
            hkey_inproc, PCWSTR(threading_name.as_ptr()), Some(0), REG_SZ,
            Some(std::slice::from_raw_parts(threading_val.as_ptr() as *const u8, threading_val.len() * 2)),
        ).ok()?;
        let _ = RegCloseKey(hkey_inproc);
    }
    Ok(())
}

/// COM InprocServer32 レジストリ登録解除
fn unregister_com_server() -> Result<()> {
    unsafe {
        let clsid_str = guid_to_string(&CLSID_TRY_CODE_IME);

        // InprocServer32 サブキーを先に削除
        let inproc_subkey = format!("CLSID\\{clsid_str}\\InprocServer32");
        let inproc_w: Vec<u16> = inproc_subkey.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = RegDeleteKeyW(HKEY_CLASSES_ROOT, PCWSTR(inproc_w.as_ptr()));

        // CLSID サブキーを削除
        let subkey = format!("CLSID\\{clsid_str}");
        let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = RegDeleteKeyW(HKEY_CLASSES_ROOT, PCWSTR(subkey_w.as_ptr()));
    }
    Ok(())
}

/// サーバー登録
pub fn register_server() -> Result<()> {
    let dll_path = get_dll_path()?;

    // 1. COM InprocServer32 レジストリ登録
    register_com_server(&dll_path)?;

    unsafe {
        // 2. カテゴリマネージャでTIPカテゴリに登録
        let cat_mgr: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        cat_mgr.RegisterCategory(
            &CLSID_TRY_CODE_IME,
            &GUID_TFCAT_TIP_KEYBOARD,
            &CLSID_TRY_CODE_IME,
        )?;
        // UWP/Immersive アプリ（スタートメニュー検索等）対応
        cat_mgr.RegisterCategory(
            &CLSID_TRY_CODE_IME,
            &GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
            &CLSID_TRY_CODE_IME,
        )?;
        // システムトレイ対応
        cat_mgr.RegisterCategory(
            &CLSID_TRY_CODE_IME,
            &GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
            &CLSID_TRY_CODE_IME,
        )?;

        // 3. プロファイル登録（ITfInputProcessorProfileMgr: Immersive対応に必要）
        let profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;

        let display_name: Vec<u16> = IME_DISPLAY_NAME.encode_utf16().collect();
        let icon_file: Vec<u16> = dll_path.encode_utf16().collect();

        profile_mgr.RegisterProfile(
            &CLSID_TRY_CODE_IME,
            LANGID_JAPANESE,
            &GUID_PROFILE,
            &display_name,
            &icon_file,
            0,    // iconIndex
            windows::Win32::UI::Input::KeyboardAndMouse::HKL::default(), // hklSubstitute
            0,    // dwPreferredLayout
            true, // bEnabledByDefault
            0,    // dwFlags
        )?;
    }

    log::info!("TryCode IME registered successfully");
    Ok(())
}

/// サーバー登録解除
pub fn unregister_server() -> Result<()> {
    unsafe {
        // カテゴリ解除
        let cat_mgr: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        let _ = cat_mgr.UnregisterCategory(
            &CLSID_TRY_CODE_IME,
            &GUID_TFCAT_TIP_KEYBOARD,
            &CLSID_TRY_CODE_IME,
        );
        let _ = cat_mgr.UnregisterCategory(
            &CLSID_TRY_CODE_IME,
            &GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
            &CLSID_TRY_CODE_IME,
        );
        let _ = cat_mgr.UnregisterCategory(
            &CLSID_TRY_CODE_IME,
            &GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
            &CLSID_TRY_CODE_IME,
        );

        // プロファイル解除
        let profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
        let _ = profile_mgr.UnregisterProfile(
            &CLSID_TRY_CODE_IME,
            LANGID_JAPANESE,
            &GUID_PROFILE,
            0,
        );
    }

    // COM レジストリ解除
    let _ = unregister_com_server();

    log::info!("TryCode IME unregistered successfully");
    Ok(())
}

/// DLLファイルのパスを取得
fn get_dll_path() -> Result<String> {
    let mut buf = vec![0u16; 260];
    let len = unsafe { GetModuleFileNameW(Some(dll_module()), &mut buf) } as usize;
    if len == 0 {
        return Err(Error::from_hresult(HRESULT(-1)));
    }
    Ok(String::from_utf16_lossy(&buf[..len]))
}
