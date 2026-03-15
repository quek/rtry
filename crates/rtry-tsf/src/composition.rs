//! 合成文字列管理
//!
//! Arc<Mutex<>> で共有し、EditSession内からもcompositionを更新可能にする。

use std::sync::{Arc, Mutex};
use windows::Win32::UI::TextServices::*;

/// 合成文字列の状態管理 (スレッドセーフ共有用)
#[derive(Clone)]
pub struct SharedComposition {
    inner: Arc<Mutex<Option<ITfComposition>>>,
}

impl SharedComposition {
    pub fn new() -> Self {
        SharedComposition {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_composing(&self) -> bool {
        self.inner.lock().unwrap().is_some()
    }

    pub fn set(&self, composition: ITfComposition) {
        *self.inner.lock().unwrap() = Some(composition);
    }

    pub fn take(&self) -> Option<ITfComposition> {
        self.inner.lock().unwrap().take()
    }

    pub fn clear(&self) {
        *self.inner.lock().unwrap() = None;
    }

    pub fn get(&self) -> Option<ITfComposition> {
        self.inner.lock().unwrap().clone()
    }
}

impl Default for SharedComposition {
    fn default() -> Self {
        Self::new()
    }
}
