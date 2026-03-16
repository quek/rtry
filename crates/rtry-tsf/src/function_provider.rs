//! ITfFunctionProvider / ITfFnConfigure 実装
//!
//! Windows の「設定 > 時刻と言語 > キーボード」からIMEオプションを開いた際に
//! システムが呼び出す標準インターフェース。設定画面（rtry-config.exe）を起動する。

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;

use crate::language_bar::launch_config;
use crate::CLSID_TRY_CODE_IME;

#[implement(ITfFunctionProvider, ITfFnConfigure)]
pub struct TryCodeFunctionProvider;

impl ITfFunctionProvider_Impl for TryCodeFunctionProvider_Impl {
    fn GetType(&self) -> Result<GUID> {
        Ok(CLSID_TRY_CODE_IME)
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("try-code"))
    }

    fn GetFunction(&self, _rguid: *const GUID, riid: *const GUID) -> Result<IUnknown> {
        unsafe {
            let iid = &*riid;
            if *iid == ITfFnConfigure::IID {
                let this: IUnknown = self.to_interface();
                Ok(this)
            } else {
                Err(E_NOINTERFACE.into())
            }
        }
    }
}

impl ITfFunction_Impl for TryCodeFunctionProvider_Impl {
    fn GetDisplayName(&self) -> Result<BSTR> {
        Ok(BSTR::from("try-code 設定"))
    }
}

impl ITfFnConfigure_Impl for TryCodeFunctionProvider_Impl {
    fn Show(
        &self,
        _hwndparent: HWND,
        _langid: u16,
        _rguidprofile: *const GUID,
    ) -> Result<()> {
        launch_config();
        Ok(())
    }
}
