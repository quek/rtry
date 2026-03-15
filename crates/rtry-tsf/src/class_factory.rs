//! COM IClassFactory 実装

use crate::text_service::TryCodeTextService;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;

#[implement(IClassFactory)]
pub struct TryCodeClassFactory;

impl IClassFactory_Impl for TryCodeClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Ref<'_, IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut std::ffi::c_void,
    ) -> Result<()> {
        unsafe {
            if ppvobject.is_null() {
                return Err(Error::from_hresult(E_POINTER));
            }
            *ppvobject = std::ptr::null_mut();

            if !punkouter.is_null() {
                return Err(Error::from_hresult(CLASS_E_NOAGGREGATION));
            }

            let service = TryCodeTextService::new();
            let unknown: IUnknown = service.into();
            let hr = unknown.query(&*riid, ppvobject);
            hr.ok()
        }
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        if flock.as_bool() {
            crate::dll_add_ref();
        } else {
            crate::dll_release();
        }
        Ok(())
    }
}
