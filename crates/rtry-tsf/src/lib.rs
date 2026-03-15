//! rtry-tsf: try-code Windows IME (TSF Text Input Processor)

mod class_factory;
mod text_service;
mod key_handler;
mod edit_session;
mod composition;
mod language_bar;
mod register;
mod stroke_help;
mod candidate_window;

/// デバッグログのパスを決定（%TEMP%\rtry_debug.log）
/// Mozc (glog) と同様に %TEMP% を使用。通常プロセス・AppContainer 両方から書き込み可能
fn debug_log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("rtry_debug.log")
}

/// デバッグログをファイルに出力するマクロ
macro_rules! debug_log {
    ($($arg:tt)*) => {{
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(crate::debug_log_path())
        {
            let _ = writeln!(f, "{}", format!($($arg)*));
        }
    }};
}
pub(crate) use debug_log;

use std::sync::atomic::{AtomicU32, Ordering};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::SystemServices::*;

/// DLLのグローバル参照カウント
static DLL_REF_COUNT: AtomicU32 = AtomicU32::new(0);

/// DLLインスタンスハンドル
static mut DLL_INSTANCE: HMODULE = HMODULE(std::ptr::null_mut());

/// IMEのCLSID
pub const CLSID_TRY_CODE_IME: GUID = GUID::from_u128(0xb7e6f9a1_3c4d_4e5f_8a9b_1c2d3e4f5a6b);

/// 言語プロファイルのGUID
pub const GUID_PROFILE: GUID = GUID::from_u128(0xc8f7a0b2_4d5e_5f60_9bac_2d3e4f5a6b7c);


pub(crate) fn dll_add_ref() {
    DLL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn dll_release() {
    DLL_REF_COUNT.fetch_sub(1, Ordering::SeqCst);
}

pub(crate) fn dll_module() -> HMODULE {
    unsafe { DLL_INSTANCE }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllMain(
    hinst: HINSTANCE,
    reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            unsafe { DLL_INSTANCE = HMODULE(hinst.0); }
            debug_log!("DllMain ATTACH, pid={}", std::process::id());
        }
        DLL_PROCESS_DETACH => {}
        _ => {}
    }
    TRUE
}

#[unsafe(no_mangle)]
pub extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut std::ffi::c_void,
) -> HRESULT {
    unsafe {
        if ppv.is_null() {
            return E_INVALIDARG;
        }
        *ppv = std::ptr::null_mut();

        if rclsid.is_null() || riid.is_null() {
            return E_INVALIDARG;
        }

        let clsid = &*rclsid;
        if *clsid != CLSID_TRY_CODE_IME {
            return CLASS_E_CLASSNOTAVAILABLE;
        }

        let factory: IClassFactory = class_factory::TryCodeClassFactory.into();
        factory.query(&*riid, ppv)
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if DLL_REF_COUNT.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    match register::register_server() {
        Ok(()) => S_OK,
        Err(_) => E_FAIL,
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    match register::unregister_server() {
        Ok(()) => S_OK,
        Err(_) => E_FAIL,
    }
}
