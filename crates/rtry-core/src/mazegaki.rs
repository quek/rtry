//! 交ぜ書き変換辞書
//!
//! SKK辞書形式の辞書ファイルを読み込み、読みから候補を検索する。

use std::collections::HashMap;
use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MazegakiError {
    #[error("failed to read dictionary file: {0}")]
    Io(#[from] std::io::Error),
}

/// SKK辞書形式の交ぜ書き辞書
#[derive(Debug)]
pub struct MazegakiDictionary {
    /// 読み → 候補リスト
    entries: HashMap<String, Vec<String>>,
    /// 読みの最大文字数（検索範囲制限用）
    max_reading_len: usize,
}

impl MazegakiDictionary {
    /// 辞書ファイルを読み込む（UTF-8 SKK形式）
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, MazegakiError> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::parse(&content))
    }

    /// 辞書テキストをパース
    pub fn parse(content: &str) -> Self {
        let mut entries: HashMap<String, Vec<String>> = HashMap::new();
        let mut max_reading_len = 0usize;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }

            // SKK形式: 読み /候補1/候補2/候補3/
            let Some((reading, rest)) = line.split_once(' ') else {
                continue;
            };
            let reading = reading.trim();
            if reading.is_empty() {
                continue;
            }

            let candidates: Vec<String> = rest
                .trim()
                .trim_start_matches('/')
                .trim_end_matches('/')
                .split('/')
                .filter(|s| !s.is_empty())
                .map(|s| {
                    // アノテーション（;以降）を除去
                    s.split_once(';').map_or(s, |(word, _)| word).to_string()
                })
                .collect();

            if !candidates.is_empty() {
                max_reading_len = max_reading_len.max(reading.chars().count());
                entries.insert(reading.to_string(), candidates);
            }
        }

        MazegakiDictionary {
            entries,
            max_reading_len,
        }
    }

    /// 読みから候補を検索
    pub fn lookup(&self, reading: &str) -> Option<&[String]> {
        self.entries.get(reading).map(|v| v.as_slice())
    }

    /// テキスト末尾から最長一致検索
    ///
    /// 戻り値: (読みの文字数, 読み, 候補リスト)
    pub fn find_longest_match(&self, text: &str) -> Option<(usize, String, Vec<String>)> {
        let chars: Vec<char> = text.chars().collect();
        let max_len = chars.len().min(self.max_reading_len);

        // 長い方から試す
        for len in (1..=max_len).rev() {
            let reading: String = chars[chars.len() - len..].iter().collect();
            if let Some(candidates) = self.entries.get(&reading) {
                return Some((len, reading, candidates.clone()));
            }
        }
        None
    }

    /// 辞書のエントリ数
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DICT: &str = "\
; test dictionary
きしゃ /記者/汽車/貴社/
かんじ /漢字/感じ;かんじる/幹事/
き /木/気/
きし /岸/騎士/
";

    #[test]
    fn test_parse() {
        let dict = MazegakiDictionary::parse(TEST_DICT);
        assert_eq!(dict.len(), 4);
        assert_eq!(dict.max_reading_len, 3); // "きしゃ" = 3文字
    }

    #[test]
    fn test_lookup() {
        let dict = MazegakiDictionary::parse(TEST_DICT);

        let candidates = dict.lookup("きしゃ").unwrap();
        assert_eq!(candidates, &["記者", "汽車", "貴社"]);

        // アノテーション除去
        let candidates = dict.lookup("かんじ").unwrap();
        assert_eq!(candidates, &["漢字", "感じ", "幹事"]);

        assert!(dict.lookup("ない").is_none());
    }

    #[test]
    fn test_longest_match() {
        let dict = MazegakiDictionary::parse(TEST_DICT);

        // "あきしゃ" → "きしゃ"(3文字) が最長
        let result = dict.find_longest_match("あきしゃ").unwrap();
        assert_eq!(result.0, 3);
        assert_eq!(result.1, "きしゃ");
        assert_eq!(result.2, &["記者", "汽車", "貴社"]);

        // "きし" → "きし"(2文字) が最長
        let result = dict.find_longest_match("きし").unwrap();
        assert_eq!(result.0, 2);
        assert_eq!(result.1, "きし");

        // "き" → "き"(1文字)
        let result = dict.find_longest_match("き").unwrap();
        assert_eq!(result.0, 1);

        // マッチなし
        assert!(dict.find_longest_match("xyz").is_none());
        assert!(dict.find_longest_match("").is_none());
    }

    #[test]
    fn test_empty_and_comment_lines() {
        let dict = MazegakiDictionary::parse("; comment\n\n  \n");
        assert!(dict.is_empty());
    }
}
