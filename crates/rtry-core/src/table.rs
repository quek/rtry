//! try-code テーブルファイルのパーサーとルックアップ
//!
//! try.tbl のフォーマット:
//! - コメント行: `;;` で始まる
//! - ヘッダ定義: `#define table-name`, `#define key-layout`, `#define prefix`
//! - テーブル本体: ネストされた `{}` 内に文字マッピング
//!
//! テーブル構造 (depth表記):
//! - depth 1: 最外層 `{}`
//! - depth 2: 基本テーブルの各行 (40個のセクション、各40エントリ)
//!            + 拡張テーブル (1個のセクション、内部にdepth-3サブブロック40個)
//! - depth 3: 拡張テーブルの各行 (40個のサブブロック、各40エントリ)

use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TableError {
    #[error("failed to read table file: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
}

/// QWERTY-JIS配列における40キーの定義 (4行 × 10列)
pub const QWERTY_KEYS: [char; 40] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0',
    'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p',
    'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';',
    'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/',
];

/// テーブルエントリの種類
#[derive(Debug, Clone, PartialEq)]
pub enum TableEntry {
    Char(String),
    Empty,
    Special(SpecialFunction),
}

/// ストロークシーケンス（逆引き用）
#[derive(Debug, Clone, PartialEq)]
pub enum StrokeSequence {
    /// 2打鍵 (first_index, second_index)
    TwoStroke(usize, usize),
    /// 3打鍵 Space前置 (first_index, second_index)
    ThreeStroke(usize, usize),
}

impl StrokeSequence {
    /// 表示用文字列に変換（例: "a → k", "Space → x → y"）
    pub fn to_display_string(&self, keys: &[char; 40]) -> String {
        match self {
            StrokeSequence::TwoStroke(first, second) => {
                let k1 = keys.get(*first).unwrap_or(&'?');
                let k2 = keys.get(*second).unwrap_or(&'?');
                format!("{} → {}", k1, k2)
            }
            StrokeSequence::ThreeStroke(first, second) => {
                let k1 = keys.get(*first).unwrap_or(&'?');
                let k2 = keys.get(*second).unwrap_or(&'?');
                format!("Space → {} → {}", k1, k2)
            }
        }
    }
}

/// 特殊機能の定義
#[derive(Debug, Clone, PartialEq)]
pub enum SpecialFunction {
    ThreeStrokePrefix,
    BushuCompose,
    MazegakiConvert,
    HistoryInput,
    Cancel,
    PostBushuCompose,
    PostMazegaki(u8),
    CharHelp(bool),
    KatakanaToggle,
    FullwidthToggle,
    PunctuationToggle,
    Virtual,
    StrokeMarker(u8),
    Marker(String),
}

/// try-code テーブル
#[derive(Debug)]
pub struct TryCodeTable {
    pub name: String,
    pub key_layout: String,
    /// 2打鍵テーブル: base_table[first_key][second_key]
    pub base_table: Vec<Vec<TableEntry>>,
    /// 3打鍵拡張テーブル (Space前置): ext_table[first_key][second_key]
    pub ext_table: Vec<Vec<TableEntry>>,
    key_index: HashMap<char, usize>,
    /// 逆引きテーブル: 文字 → ストロークシーケンスのリスト
    reverse_map: HashMap<String, Vec<StrokeSequence>>,
    /// 実際に使用中の40キーレイアウト
    keys: [char; 40],
}

impl TryCodeTable {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, TableError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self, TableError> {
        let mut name = String::from("Try-Code");
        let mut key_layout = String::from("QWERTY-JIS");

        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(v) = trimmed.strip_prefix("#define table-name ") {
                name = v.to_string();
            } else if let Some(v) = trimmed.strip_prefix("#define key-layout ") {
                key_layout = v.to_string();
            }
        }

        let tokens = tokenize(content);
        let (base_table, ext_table) = parse_table(tokens)?;

        let mut key_index = HashMap::new();
        for (i, &key) in QWERTY_KEYS.iter().enumerate() {
            key_index.insert(key, i);
        }

        let reverse_map = Self::build_reverse_map(&base_table, &ext_table);
        Ok(TryCodeTable { name, key_layout, base_table, ext_table, key_index, reverse_map, keys: QWERTY_KEYS })
    }

    /// 逆引きテーブルを構築
    fn build_reverse_map(
        base_table: &[Vec<TableEntry>],
        ext_table: &[Vec<TableEntry>],
    ) -> HashMap<String, Vec<StrokeSequence>> {
        let mut map: HashMap<String, Vec<StrokeSequence>> = HashMap::new();
        for (i, row) in base_table.iter().enumerate() {
            for (j, entry) in row.iter().enumerate() {
                if let TableEntry::Char(s) = entry {
                    map.entry(s.clone())
                        .or_default()
                        .push(StrokeSequence::TwoStroke(i, j));
                }
            }
        }
        for (i, row) in ext_table.iter().enumerate() {
            for (j, entry) in row.iter().enumerate() {
                if let TableEntry::Char(s) = entry {
                    map.entry(s.clone())
                        .or_default()
                        .push(StrokeSequence::ThreeStroke(i, j));
                }
            }
        }
        map
    }

    pub fn key_to_index(&self, key: char) -> Option<usize> {
        self.key_index.get(&key.to_ascii_lowercase()).copied()
    }

    pub fn lookup_2stroke(&self, first: usize, second: usize) -> Option<&TableEntry> {
        self.base_table.get(first)
            .and_then(|row| row.get(second))
            .filter(|e| !matches!(e, TableEntry::Empty))
    }

    pub fn lookup_3stroke(&self, first: usize, second: usize) -> Option<&TableEntry> {
        self.ext_table.get(first)
            .and_then(|row| row.get(second))
            .filter(|e| !matches!(e, TableEntry::Empty))
    }

    pub fn lookup_by_keys(&self, first: char, second: char) -> Option<&TableEntry> {
        let i = self.key_to_index(first)?;
        let j = self.key_to_index(second)?;
        self.lookup_2stroke(i, j)
    }

    pub fn lookup_by_keys_ext(&self, first: char, second: char) -> Option<&TableEntry> {
        let i = self.key_to_index(first)?;
        let j = self.key_to_index(second)?;
        self.lookup_3stroke(i, j)
    }

    /// 文字からストロークシーケンスを逆引き
    pub fn reverse_lookup(&self, ch: &str) -> &[StrokeSequence] {
        self.reverse_map.get(ch).map_or(&[], |v| v.as_slice())
    }

    /// カスタムキーレイアウトを設定（key_index を再構築）
    pub fn set_key_layout(&mut self, layout: [char; 40]) {
        self.keys = layout;
        self.key_index.clear();
        for (i, &key) in layout.iter().enumerate() {
            self.key_index.insert(key, i);
        }
    }

    /// インデックスからキー文字を取得
    pub fn key_at(&self, index: usize) -> Option<char> {
        self.keys.get(index).copied()
    }

    /// 現在のキーレイアウトを取得
    pub fn key_layout_40(&self) -> &[char; 40] {
        &self.keys
    }
}

// --- Tokenizer ---

#[derive(Debug, Clone)]
enum Token {
    OpenBrace,
    CloseBrace,
    Comma,
    Str(String),
    Marker(String),
    Empty,
}

fn tokenize(content: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut in_table = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(";;") || trimmed.starts_with("#define") || trimmed.is_empty() {
            continue;
        }

        let mut chars = trimmed.chars().peekable();
        while let Some(&ch) = chars.peek() {
            match ch {
                '{' => {
                    in_table = true;
                    tokens.push(Token::OpenBrace);
                    chars.next();
                }
                '}' => {
                    tokens.push(Token::CloseBrace);
                    chars.next();
                }
                ',' => {
                    if let Some(last) = tokens.last() {
                        if matches!(last, Token::Comma | Token::OpenBrace) {
                            tokens.push(Token::Empty);
                        }
                    }
                    tokens.push(Token::Comma);
                    chars.next();
                }
                '"' => {
                    chars.next();
                    let mut s = String::new();
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c == '"' { break; }
                        s.push(c);
                    }
                    tokens.push(Token::Str(s));
                }
                '@' => {
                    chars.next();
                    let mut marker = String::from("@");
                    while let Some(&c) = chars.peek() {
                        if c.is_alphanumeric() || c == '!' {
                            marker.push(c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    tokens.push(Token::Marker(marker));
                }
                ' ' | '\t' => { chars.next(); }
                _ => {
                    let mut s = String::new();
                    while let Some(&c) = chars.peek() {
                        if matches!(c, ',' | '{' | '}' | ' ' | '\t') { break; }
                        s.push(c);
                        chars.next();
                    }
                    if !s.is_empty() && in_table {
                        tokens.push(Token::Str(s));
                    }
                }
            }
        }
    }
    tokens
}

// --- Parser ---

/// トークン列からテーブルをパース
///
/// 構造: depth-1の`{}` 内に depth-2セクションが並ぶ。
/// - 最初の40個の depth-2セクション: 基本テーブルの各行 (40エントリ + @vマーカー)
/// - 最後の1個の depth-2セクション: 拡張テーブル (内部にdepth-3サブブロック40個)
fn parse_table(tokens: Vec<Token>) -> Result<(Vec<Vec<TableEntry>>, Vec<Vec<TableEntry>>), TableError> {
    // depth-2 セクションの内容を収集
    let mut sections: Vec<SectionContent> = Vec::new();
    let mut depth = 0;
    let mut current_entries: Vec<TableEntry> = Vec::new();
    let mut has_subblocks = false;
    let mut sub_entries: Vec<Vec<TableEntry>> = Vec::new();
    let mut sub_current: Vec<TableEntry> = Vec::new();

    for token in &tokens {
        match token {
            Token::OpenBrace => {
                depth += 1;
                if depth == 2 {
                    current_entries.clear();
                    has_subblocks = false;
                    sub_entries.clear();
                } else if depth == 3 {
                    has_subblocks = true;
                    sub_current.clear();
                }
            }
            Token::CloseBrace => {
                if depth == 3 {
                    sub_entries.push(std::mem::take(&mut sub_current));
                } else if depth == 2 {
                    if has_subblocks {
                        sections.push(SectionContent::Nested(std::mem::take(&mut sub_entries)));
                    } else {
                        sections.push(SectionContent::Flat(std::mem::take(&mut current_entries)));
                    }
                }
                depth -= 1;
            }
            _ => {
                let entry = token_to_entry(token);
                if let Some(entry) = entry {
                    if depth == 3 {
                        sub_current.push(entry);
                    } else if depth == 2 {
                        current_entries.push(entry);
                    }
                }
            }
        }
    }

    // セクションをbase_tableとext_tableに分類
    let mut base_table: Vec<Vec<TableEntry>> = Vec::new();
    let mut ext_table: Vec<Vec<TableEntry>> = Vec::new();

    for section in &sections {
        match section {
            SectionContent::Flat(entries) => {
                // フラットセクション = 基本テーブルの1行
                let row = entries_to_row(entries);
                base_table.push(row);
            }
            SectionContent::Nested(sub_blocks) => {
                // ネストセクション = 拡張テーブルの複数行
                for sub in sub_blocks {
                    let row = entries_to_row(sub);
                    ext_table.push(row);
                }
            }
        }
    }

    // パディング
    while base_table.len() < 40 {
        base_table.push(vec![TableEntry::Empty; 40]);
    }
    while ext_table.len() < 40 {
        ext_table.push(vec![TableEntry::Empty; 40]);
    }

    // 40行を超える場合は切り詰め
    base_table.truncate(40);
    ext_table.truncate(40);

    Ok((base_table, ext_table))
}

enum SectionContent {
    Flat(Vec<TableEntry>),
    Nested(Vec<Vec<TableEntry>>),
}

/// エントリリストから40要素の行を生成
fn entries_to_row(entries: &[TableEntry]) -> Vec<TableEntry> {
    // @v マーカーとその後の空白を除外
    let filtered: Vec<&TableEntry> = entries.iter()
        .filter(|e| !matches!(e, TableEntry::Special(SpecialFunction::Virtual)))
        .collect();

    let mut row = Vec::with_capacity(40);
    for i in 0..40 {
        if i < filtered.len() {
            row.push(filtered[i].clone());
        } else {
            row.push(TableEntry::Empty);
        }
    }
    row
}

fn token_to_entry(token: &Token) -> Option<TableEntry> {
    match token {
        Token::Str(s) => Some(TableEntry::Char(s.clone())),
        Token::Marker(m) => Some(parse_marker(m)),
        Token::Empty => Some(TableEntry::Empty),
        _ => None,
    }
}

fn parse_marker(marker: &str) -> TableEntry {
    match marker {
        "@v" => TableEntry::Special(SpecialFunction::Virtual),
        "@b" => TableEntry::Special(SpecialFunction::BushuCompose),
        "@m" => TableEntry::Special(SpecialFunction::MazegakiConvert),
        "@!" => TableEntry::Special(SpecialFunction::Cancel),
        "@B" => TableEntry::Special(SpecialFunction::PostBushuCompose),
        "@h" => TableEntry::Special(SpecialFunction::CharHelp(false)),
        "@H" => TableEntry::Special(SpecialFunction::CharHelp(true)),
        "@p" => TableEntry::Special(SpecialFunction::PunctuationToggle),
        "@Z" => TableEntry::Special(SpecialFunction::FullwidthToggle),
        "@K" => TableEntry::Special(SpecialFunction::KatakanaToggle),
        "@q" => TableEntry::Special(SpecialFunction::HistoryInput),
        _ if marker.starts_with('@') && marker.len() == 2 => {
            if let Some(n) = marker.chars().nth(1).and_then(|c| c.to_digit(10)) {
                TableEntry::Special(SpecialFunction::StrokeMarker(n as u8))
            } else {
                TableEntry::Special(SpecialFunction::Marker(marker.to_string()))
            }
        }
        _ => TableEntry::Special(SpecialFunction::Marker(marker.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_table() -> TryCodeTable {
        let table_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/try.tbl");
        TryCodeTable::load(table_path).expect("failed to load try.tbl")
    }

    #[test]
    fn test_load_table() {
        let table = load_table();

        assert_eq!(table.name, "Try-Code");
        assert_eq!(table.key_layout, "QWERTY-JIS");
        assert_eq!(table.base_table.len(), 40, "base_table has {} rows", table.base_table.len());
        assert_eq!(table.ext_table.len(), 40, "ext_table has {} rows", table.ext_table.len());

        for (i, row) in table.base_table.iter().enumerate() {
            assert_eq!(row.len(), 40, "base_table row {} has {} entries", i, row.len());
        }
        for (i, row) in table.ext_table.iter().enumerate() {
            assert_eq!(row.len(), 40, "ext_table row {} has {} entries", i, row.len());
        }
    }

    #[test]
    fn test_key_index() {
        let table = load_table();
        assert_eq!(table.key_to_index('1'), Some(0));
        assert_eq!(table.key_to_index('q'), Some(10));
        assert_eq!(table.key_to_index('a'), Some(20));
        assert_eq!(table.key_to_index('z'), Some(30));
        assert_eq!(table.key_to_index(';'), Some(29));
    }

    #[test]
    fn test_base_table_content() {
        let table = load_table();

        // base_table の各行に文字エントリがあることを確認
        let mut total_chars = 0;
        for i in 0..40 {
            for j in 0..40 {
                if let Some(TableEntry::Char(_)) = table.lookup_2stroke(i, j) {
                    total_chars += 1;
                }
            }
        }
        println!("Total character entries in base_table: {}", total_chars);
        // T-Code基本テーブルには約1000文字以上あるはず
        assert!(total_chars > 500, "base_table has only {} characters", total_chars);
    }

    #[test]
    fn test_ext_table_content() {
        let table = load_table();

        let mut total_chars = 0;
        for i in 0..40 {
            for j in 0..40 {
                if let Some(TableEntry::Char(_)) = table.lookup_3stroke(i, j) {
                    total_chars += 1;
                }
            }
        }
        println!("Total character entries in ext_table: {}", total_chars);
        // 拡張テーブルにも文字があるはず
        assert!(total_chars > 100, "ext_table has only {} characters", total_chars);
    }

    #[test]
    fn test_known_characters() {
        let table = load_table();

        // try.tbl から確認できる既知の文字をテスト
        // base_table row 0 (key '1'), columns 10-19 (keys q-p):
        // "ヲ","ゥ","ヴ","ヂ","ヅ","簡","承","快","包","唱"
        let entry = table.lookup_2stroke(0, 10);
        assert_eq!(entry, Some(&TableEntry::Char("ヲ".to_string())),
            "base[0][10] should be ヲ, got {:?}", entry);

        let entry = table.lookup_2stroke(0, 15);
        assert_eq!(entry, Some(&TableEntry::Char("簡".to_string())),
            "base[0][15] should be 簡, got {:?}", entry);
    }

    #[test]
    fn test_charhelp_positions() {
        let table = load_table();
        // try.tbl: 55 = @H = CharHelp(true)
        // key '5' = index 4
        let entry_55 = &table.base_table[4][4];
        assert_eq!(*entry_55, TableEntry::Special(SpecialFunction::CharHelp(true)),
            "base_table[4][4] (55) should be CharHelp(true), got {:?}", entry_55);

        // try.tbl: 44 = @h = CharHelp(false) （再表示）
        // key '4' = index 3
        let entry_44 = &table.base_table[3][3];
        assert_eq!(*entry_44, TableEntry::Special(SpecialFunction::CharHelp(false)),
            "base_table[3][3] (44) should be CharHelp(false), got {:?}", entry_44);
    }

    #[test]
    fn test_reverse_lookup() {
        let table = load_table();

        // "ヲ" は base[0][10] → TwoStroke(0, 10) = "1 → q"
        let strokes = table.reverse_lookup("ヲ");
        assert!(!strokes.is_empty(), "ヲ should have strokes");
        assert!(strokes.contains(&StrokeSequence::TwoStroke(0, 10)),
            "ヲ should have TwoStroke(0, 10), got {:?}", strokes);
        assert_eq!(strokes[0].to_display_string(&QWERTY_KEYS), "1 → q");

        // 存在しない文字
        let strokes = table.reverse_lookup("㍻");
        assert!(strokes.is_empty());
    }
}
