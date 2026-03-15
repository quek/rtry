//! 部首合成変換
//!
//! 2つの漢字・部首から合成漢字を生成する。
//! 例: 木 + 木 → 林, 木 + 林 → 森

use std::collections::HashMap;

/// 部首合成テーブル
pub struct BushuComposer {
    /// (部首1, 部首2) → 合成結果
    compose_table: HashMap<(String, String), String>,
}

impl BushuComposer {
    pub fn new() -> Self {
        let mut composer = BushuComposer {
            compose_table: HashMap::new(),
        };
        composer.load_default_rules();
        composer
    }

    /// 2つの文字から合成を試みる
    pub fn compose(&self, first: &str, second: &str) -> Option<&String> {
        self.compose_table
            .get(&(first.to_string(), second.to_string()))
            .or_else(|| {
                // 逆順でも試す
                self.compose_table
                    .get(&(second.to_string(), first.to_string()))
            })
    }

    /// デフォルトの部首合成ルールをロード
    fn load_default_rules(&mut self) {
        // 基本的な合成ルール
        let rules = [
            ("木", "木", "林"),
            ("木", "林", "森"),
            ("日", "月", "明"),
            ("女", "子", "好"),
            ("口", "口", "回"),
            ("人", "人", "従"),
            ("山", "山", "出"),
            ("火", "火", "炎"),
            ("田", "力", "男"),
            ("言", "寺", "詩"),
            ("糸", "泉", "線"),
            ("金", "同", "銅"),
            ("イ", "立", "位"),
            ("サ", "化", "花"),
        ];

        for (a, b, result) in &rules {
            self.compose_table.insert(
                (a.to_string(), b.to_string()),
                result.to_string(),
            );
        }
    }

    /// 外部ファイルから合成ルールをロード
    pub fn load_from_file(&mut self, _path: &str) -> Result<(), std::io::Error> {
        // TODO: tc-bushu.rev 等のファイルからルールをロード
        Ok(())
    }
}

impl Default for BushuComposer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_composition() {
        let composer = BushuComposer::new();
        assert_eq!(composer.compose("木", "木"), Some(&"林".to_string()));
        assert_eq!(composer.compose("日", "月"), Some(&"明".to_string()));
    }

    #[test]
    fn test_reverse_composition() {
        let composer = BushuComposer::new();
        // 逆順でも合成できる
        assert_eq!(composer.compose("月", "日"), Some(&"明".to_string()));
    }

    #[test]
    fn test_unknown_composition() {
        let composer = BushuComposer::new();
        assert_eq!(composer.compose("犬", "猫"), None);
    }
}
