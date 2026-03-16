//! 設定管理

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::table::QWERTY_KEYS;

/// アプリケーション設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// テーブルファイルのパス
    pub table_path: PathBuf,
    /// キーレイアウト
    pub key_layout: String,
    /// ヒストリの最大保持数
    pub history_max_size: usize,
    /// 句読点スタイル (true: 、。 false: ，．)
    pub use_japanese_punctuation: bool,
    /// IME ONインジケーターを表示するか
    #[serde(default = "default_true")]
    pub show_ime_indicator: bool,
    /// 3ストローク入力のプレフィックスキー（デフォルト: Space）
    #[serde(default = "default_space")]
    pub ext_prefix_key: char,
    /// カスタム40キーレイアウト（None の場合は QWERTY_KEYS を使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_layout_40: Option<Vec<char>>,
}

fn default_true() -> bool {
    true
}

fn default_space() -> char {
    ' '
}

impl Default for Config {
    fn default() -> Self {
        Config {
            table_path: PathBuf::from("try.tbl"),
            key_layout: "QWERTY-JIS".to_string(),
            history_max_size: 100,
            use_japanese_punctuation: true,
            show_ime_indicator: true,
            ext_prefix_key: ' ',
            key_layout_40: None,
        }
    }
}

impl Config {
    /// 設定ディレクトリのパスを取得
    pub fn config_dir() -> Option<PathBuf> {
        std::env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("rtry"))
    }

    /// 設定ファイルのパスを取得
    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join("config.json"))
    }

    /// 設定をファイルから読み込み
    pub fn load() -> Self {
        Self::config_path()
            .and_then(|path| Self::load_from(&path).ok())
            .unwrap_or_default()
    }

    /// 指定パスから読み込み
    pub fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// 設定をファイルに保存
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(path) = Self::config_path() {
            self.save_to(&path)
        } else {
            Err("could not determine config directory".into())
        }
    }

    /// 指定パスに保存
    pub fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// 有効なキーレイアウトを取得（カスタム設定がなければ QWERTY_KEYS）
    pub fn effective_key_layout(&self) -> [char; 40] {
        if let Some(ref layout) = self.key_layout_40 {
            if layout.len() == 40 {
                let mut arr = [' '; 40];
                arr.copy_from_slice(layout);
                return arr;
            }
        }
        QWERTY_KEYS
    }
}
