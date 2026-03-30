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

/// 活用語尾の最大文字数（tc2 準拠）
const MAX_INFLECTION_SUFFIX: usize = 4;

/// 活用語マーカー
const INFLECTION_MARK: &str = "―";

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

    /// テキスト末尾から最長一致検索（活用語対応）
    ///
    /// 戻り値: (読みの文字数, 候補リスト)
    pub fn find_longest_match(&self, text: &str) -> Option<(usize, Vec<String>)> {
        self.find_match_impl(text, 1, usize::MAX, true)
    }

    /// current_len より短いマッチを探す（最長優先、候補が見つかるまで縮める）
    pub fn find_shorter_match(&self, text: &str, current_len: usize) -> Option<(usize, Vec<String>)> {
        if current_len <= 1 { return None; }
        self.find_match_impl(text, 1, current_len - 1, true)
    }

    /// current_len より長いマッチを探す（最短優先、候補が見つかるまで伸ばす）
    pub fn find_longer_match(&self, text: &str, current_len: usize) -> Option<(usize, Vec<String>)> {
        self.find_match_impl(text, current_len + 1, usize::MAX, false)
    }

    /// 交ぜ書き検索の内部実装
    ///
    /// min_chars..=max_chars の範囲でマッチを探す。
    /// prefer_longest=true なら最長、false なら最短を返す。
    /// 活用語が完全一致に勝つには語幹文字数 >= 完全一致の文字数が必要。
    fn find_match_impl(
        &self, text: &str, min_chars: usize, max_chars: usize, prefer_longest: bool,
    ) -> Option<(usize, Vec<String>)> {
        let char_count = text.chars().count();
        let search_len = char_count.min(self.max_reading_len + MAX_INFLECTION_SUFFIX);

        let boundaries: Vec<(usize, usize)> = text
            .char_indices()
            .rev()
            .take(search_len)
            .map(|(byte_off, _)| {
                let char_pos = text[..byte_off].chars().count();
                (byte_off, char_count - char_pos)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        // 完全一致
        let mut best_exact: Option<(usize, Vec<String>)> = None;
        for &(byte_offset, reading_chars) in &boundaries {
            if reading_chars < min_chars || reading_chars > max_chars { continue; }
            if reading_chars > self.max_reading_len { continue; }
            let reading = &text[byte_offset..];
            if let Some(candidates) = self.entries.get(reading) {
                let dominated = best_exact.as_ref().is_some_and(|b| {
                    if prefer_longest { reading_chars <= b.0 } else { reading_chars >= b.0 }
                });
                if !dominated {
                    best_exact = Some((reading_chars, candidates.clone()));
                }
            }
        }

        // 活用語
        let mut best_inflection: Option<(usize, usize, Vec<String>)> = None; // (total, stem_chars, candidates)
        for &(byte_offset, reading_chars) in &boundaries {
            if reading_chars < min_chars || reading_chars > max_chars { continue; }
            let candidate_text = &text[byte_offset..];
            let suffix_max = reading_chars.saturating_sub(1).min(MAX_INFLECTION_SUFFIX);
            let suffix_boundaries: Vec<(usize, usize)> = candidate_text
                .char_indices()
                .rev()
                .take(suffix_max)
                .map(|(byte_off, _)| {
                    let stem_chars = candidate_text[..byte_off].chars().count();
                    (byte_off, stem_chars)
                })
                .collect();

            for &(suffix_byte_offset, stem_chars) in &suffix_boundaries {
                let suffix = &candidate_text[suffix_byte_offset..];
                if !suffix.chars().all(is_hiragana) { continue; }
                let stem = &candidate_text[..suffix_byte_offset];
                let key = format!("{}{}", stem, INFLECTION_MARK);
                if let Some(dict_candidates) = self.entries.get(&key) {
                    let dominated = best_inflection.as_ref().is_some_and(|b| {
                        if prefer_longest {
                            reading_chars < b.0 || (reading_chars == b.0 && stem_chars <= b.1)
                        } else {
                            reading_chars > b.0 || (reading_chars == b.0 && stem_chars <= b.1)
                        }
                    });
                    if !dominated {
                        let candidates: Vec<String> = dict_candidates
                            .iter()
                            .map(|c| format!("{}{}", c, suffix))
                            .collect();
                        best_inflection = Some((reading_chars, stem_chars, candidates));
                    }
                }
            }
        }

        // 判定:
        // - 同じ読み長さ → 完全一致 + 活用語をマージ
        // - 活用語が長い場合、語幹文字数 >= 完全一致の文字数なら活用語が勝つ
        // - それ以外 → 完全一致が勝つ
        match (best_exact, best_inflection) {
            (Some(exact), Some((total, _stem, inf_cands))) if total == exact.0 => {
                let mut merged = exact.1;
                merged.extend(inf_cands);
                Some((exact.0, merged))
            }
            (Some(exact), Some((total, stem, candidates))) if stem >= exact.0 => {
                Some((total, candidates))
            }
            (Some(exact), _) => Some(exact),
            (None, Some((total, _, candidates))) => Some((total, candidates)),
            (None, None) => None,
        }
    }

    /// 辞書のエントリ数
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// ひらがな判定（U+3040〜U+309F）
fn is_hiragana(c: char) -> bool {
    ('\u{3040}'..='\u{309F}').contains(&c)
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
あきらめ― /諦め/
";

    #[test]
    fn test_parse() {
        let dict = MazegakiDictionary::parse(TEST_DICT);
        assert_eq!(dict.len(), 5);
        assert_eq!(dict.max_reading_len, 5); // "あきらめ―" = 5文字
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

        // 完全一致: "あきしゃ" → "きしゃ"(3文字) が最長
        let (chars, candidates) = dict.find_longest_match("あきしゃ").unwrap();
        assert_eq!(chars, 3);
        assert_eq!(candidates, &["記者", "汽車", "貴社"]);

        // 完全一致: "きし" → "きし"(2文字) が最長
        let (chars, _) = dict.find_longest_match("きし").unwrap();
        assert_eq!(chars, 2);

        // 完全一致: "き" → "き"(1文字)
        let (chars, _) = dict.find_longest_match("き").unwrap();
        assert_eq!(chars, 1);

        // マッチなし
        assert!(dict.find_longest_match("xyz").is_none());
        assert!(dict.find_longest_match("").is_none());
    }

    #[test]
    fn test_inflection_match() {
        let dict = MazegakiDictionary::parse(TEST_DICT);

        // 活用語: "あきらめる" → "あきらめ―" にマッチ、候補に語尾「る」が付く
        let (chars, candidates) = dict.find_longest_match("あきらめる").unwrap();
        assert_eq!(chars, 5);
        assert_eq!(candidates, &["諦める"]);

        // 活用語: "あきらめた" → "あきらめ―" にマッチ
        let (chars, candidates) = dict.find_longest_match("あきらめた").unwrap();
        assert_eq!(chars, 5);
        assert_eq!(candidates, &["諦めた"]);

        // 活用語: "あきらめない" → "あきらめ―" にマッチ（語尾2文字）
        let (chars, candidates) = dict.find_longest_match("あきらめない").unwrap();
        assert_eq!(chars, 6);
        assert_eq!(candidates, &["諦めない"]);

        // 活用語尾がひらがな以外ならマッチしない
        assert!(dict.find_longest_match("あきらめ漢").is_none());
    }

    #[test]
    fn test_exact_match_preferred_over_inflection() {
        // 完全一致が活用語より優先される
        let dict = MazegakiDictionary::parse("\
き /木/気/
き― /切/着/
");
        // "きる" → 完全一致「き」(1文字) ではなく活用語「き―」(2文字) が長いので活用語が勝つ
        let (chars, candidates) = dict.find_longest_match("きる").unwrap();
        assert_eq!(chars, 2);
        assert_eq!(candidates, &["切る", "着る"]);
    }

    #[test]
    fn test_inflection_wins_over_shorter_exact() {
        // 活用語(5文字)が短い完全一致(1文字)より優先される
        let dict = MazegakiDictionary::parse("\
る /縷/
あきらめ― /諦め/
");
        let (chars, candidates) = dict.find_longest_match("あきらめる").unwrap();
        assert_eq!(chars, 5);
        assert_eq!(candidates, &["諦める"]);
    }

    #[test]
    fn test_same_length_merges_exact_and_inflection() {
        // 同じ文字数なら完全一致 + 活用語をマージ（完全一致が先）
        let dict = MazegakiDictionary::parse("\
きる /着る/切る/
き― /来/
");
        let (chars, candidates) = dict.find_longest_match("きる").unwrap();
        assert_eq!(chars, 2);
        assert_eq!(candidates, &["着る", "切る", "来る"]);
    }

    #[test]
    fn test_short_stem_inflection_does_not_beat_exact() {
        // 「同時にねこ」→ 完全一致「ねこ」(2文字)が「に―」(語幹1文字)に勝つ
        let dict = MazegakiDictionary::parse("\
ねこ /猫/
に― /似/煮/
");
        let (chars, candidates) = dict.find_longest_match("同時にねこ").unwrap();
        assert_eq!(chars, 2);
        assert_eq!(candidates, &["猫"]);
    }

    #[test]
    fn test_shorter_match() {
        let dict = MazegakiDictionary::parse("\
あきらめ― /諦め/
きらめ― /煌め/
る /縷/
");
        // "あきらめる": 最長は5文字(あきらめ―+る)
        // 5文字から縮める → 4文字(きらめ―+る)
        let (chars, candidates) = dict.find_shorter_match("あきらめる", 5).unwrap();
        assert_eq!(chars, 4);
        assert_eq!(candidates, &["煌める"]);

        // 4文字から縮める → 1文字(る完全一致)
        let (chars, candidates) = dict.find_shorter_match("あきらめる", 4).unwrap();
        assert_eq!(chars, 1);
        assert_eq!(candidates, &["縷"]);

        // 1文字からは縮められない
        assert!(dict.find_shorter_match("あきらめる", 1).is_none());
    }

    #[test]
    fn test_longer_match() {
        let dict = MazegakiDictionary::parse("\
る /縷/
め― /目/
あきらめ― /諦め/
");
        // "あきらめる": 1文字(る)から伸ばす → 2文字(め―+る)
        let (chars, candidates) = dict.find_longer_match("あきらめる", 1).unwrap();
        assert_eq!(chars, 2);
        assert_eq!(candidates, &["目る"]);

        // 2文字から伸ばす → 5文字(あきらめ―+る)
        let (chars, candidates) = dict.find_longer_match("あきらめる", 2).unwrap();
        assert_eq!(chars, 5);
        assert_eq!(candidates, &["諦める"]);

        // 5文字から伸ばす → なし
        assert!(dict.find_longer_match("あきらめる", 5).is_none());
    }

    #[test]
    fn test_hoshii_merges_exact_and_inflection() {
        // 「ほしい」: 完全一致(糒)と活用語(欲しい)をマージ
        let dict = MazegakiDictionary::parse("\
ほしい /糒/
ほし― /欲し/
");
        let (chars, candidates) = dict.find_longest_match("ほしい").unwrap();
        assert_eq!(chars, 3);
        assert_eq!(candidates, &["糒", "欲しい"]);
    }

    #[test]
    fn test_empty_and_comment_lines() {
        let dict = MazegakiDictionary::parse("; comment\n\n  \n");
        assert!(dict.is_empty());
    }
}
