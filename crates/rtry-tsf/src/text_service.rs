//! ITfTextInputProcessor 実装

use std::cell::RefCell;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;

use rtry_core::engine::Engine;
use rtry_core::table::TryCodeTable;

use crate::composition::SharedComposition;
use crate::language_bar;

#[implement(ITfTextInputProcessor, ITfKeyEventSink, ITfCompositionSink)]
pub struct TryCodeTextService {
    pub(crate) thread_mgr: RefCell<Option<ITfThreadMgr>>,
    pub(crate) client_id: RefCell<u32>,
    pub(crate) engine: RefCell<Option<Engine>>,
    pub(crate) composition: SharedComposition,
    pub(crate) is_open: RefCell<bool>,
    langbar_button: RefCell<Option<ITfLangBarItemButton>>,
}

impl TryCodeTextService {
    pub fn new() -> Self {
        crate::dll_add_ref();
        TryCodeTextService {
            thread_mgr: RefCell::new(None),
            client_id: RefCell::new(0),
            engine: RefCell::new(None),
            composition: SharedComposition::new(),
            is_open: RefCell::new(true),
            langbar_button: RefCell::new(None),
        }
    }

    /// テーブルファイルを探してエンジンを初期化
    fn init_engine(&self) {
        let paths = [
            Self::dll_dir_table_path(),
            Self::appdata_table_path(),
        ];

        for path in paths.into_iter().flatten() {
            if path.exists() {
                match TryCodeTable::load(&path) {
                    Ok(table) => {
                        crate::debug_log!("Loaded try-code table from {:?}", path);
                        *self.engine.borrow_mut() = Some(Engine::new(table));
                        return;
                    }
                    Err(e) => {
                        crate::debug_log!("Failed to load table from {:?}: {}", path, e);
                    }
                }
            }
        }

        crate::debug_log!("No try-code table file found, engine not initialized");
    }

    fn dll_dir_table_path() -> Option<std::path::PathBuf> {
        use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
        let mut buf = vec![0u16; 260];
        let len = unsafe { GetModuleFileNameW(Some(crate::dll_module()), &mut buf) } as usize;
        if len == 0 { return None; }
        let dll_path = String::from_utf16_lossy(&buf[..len]);
        let path = std::path::PathBuf::from(dll_path);
        path.parent().map(|p| p.join("try.tbl"))
    }

    fn appdata_table_path() -> Option<std::path::PathBuf> {
        std::env::var("APPDATA").ok()
            .map(|p| std::path::PathBuf::from(p).join("rtry").join("try.tbl"))
    }
}

impl Drop for TryCodeTextService {
    fn drop(&mut self) {
        crate::dll_release();
    }
}

impl ITfTextInputProcessor_Impl for TryCodeTextService_Impl {
    fn Activate(&self, ptim: Ref<'_, ITfThreadMgr>, tid: u32) -> Result<()> {
        let thread_mgr = ptim.clone().ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;

        *self.client_id.borrow_mut() = tid;
        *self.thread_mgr.borrow_mut() = Some(thread_mgr.clone());

        crate::debug_log!("Activate called, tid={}", tid);
        self.init_engine();

        // キーイベントシンクの登録
        unsafe {
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
            let this_unknown: IUnknown = self.to_interface();
            let this: ITfKeyEventSink = this_unknown.cast()?;
            keystroke_mgr.AdviseKeyEventSink(tid, &this, true)?;
            crate::debug_log!("AdviseKeyEventSink succeeded");
        }

        *self.is_open.borrow_mut() = true;

        // 言語バーボタンの追加
        match language_bar::add_langbar_button(&thread_mgr) {
            Ok(button) => {
                *self.langbar_button.borrow_mut() = Some(button);
            }
            Err(_) => {}
        }

        Ok(())
    }

    fn Deactivate(&self) -> Result<()> {
        self.composition.clear();

        // 言語バーボタンの削除
        if let Some(ref thread_mgr) = *self.thread_mgr.borrow() {
            if let Some(ref button) = *self.langbar_button.borrow() {
                let _ = language_bar::remove_langbar_button(thread_mgr, button);
            }
        }
        *self.langbar_button.borrow_mut() = None;

        // キーイベントシンクの解除
        if let Some(ref thread_mgr) = *self.thread_mgr.borrow() {
            unsafe {
                let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
                let tid = *self.client_id.borrow();
                let _ = keystroke_mgr.UnadviseKeyEventSink(tid);
            }
        }

        *self.engine.borrow_mut() = None;
        *self.thread_mgr.borrow_mut() = None;
        *self.client_id.borrow_mut() = 0;

        Ok(())
    }
}

impl ITfCompositionSink_Impl for TryCodeTextService_Impl {
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Ref<'_, ITfComposition>,
    ) -> Result<()> {
        self.composition.clear();
        if let Some(ref mut engine) = *self.engine.borrow_mut() {
            engine.reset();
        }
        Ok(())
    }
}
