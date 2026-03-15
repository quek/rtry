//! 入力エンジン: 状態マシンベースの打鍵処理

use crate::history::HistoryManager;
use crate::table::{SpecialFunction, TableEntry, TryCodeTable};

/// 入力エンジンの状態
#[derive(Debug, Clone, PartialEq)]
pub enum EngineState {
    /// 初期状態 (待機中)
    Idle,
    /// 第1打鍵を受け付けた (2打鍵入力の途中)
    FirstStroke(usize),
    /// Spaceプレフィックスを受け付けた (3打鍵入力の開始)
    PrefixStroke,
    /// Spaceプレフィックス + 第1打鍵 (3打鍵入力の途中)
    ExtFirstStroke(usize),
}

/// エンジンからの出力イベント
#[derive(Debug, Clone, PartialEq)]
pub enum EngineOutput {
    /// 確定文字列を出力
    Commit(String),
    /// 合成中の文字列 (未確定)
    Composing(String),
    /// 入力をクリア
    Clear,
    /// キーを消費するが表示変更なし
    Consumed,
    /// キーを消費しない (そのまま通過)
    PassThrough,
    /// 特殊機能の実行要求
    SpecialAction(SpecialFunction),
}

/// try-code 入力エンジン
pub struct Engine {
    table: TryCodeTable,
    state: EngineState,
    history: HistoryManager,
}

impl Engine {
    pub fn new(table: TryCodeTable) -> Self {
        Engine {
            table,
            state: EngineState::Idle,
            history: HistoryManager::default(),
        }
    }

    pub fn state(&self) -> &EngineState {
        &self.state
    }

    pub fn history(&self) -> &HistoryManager {
        &self.history
    }

    /// キー入力を処理
    pub fn process_key(&mut self, key: char) -> EngineOutput {
        match self.state.clone() {
            EngineState::Idle => self.handle_idle(key),
            EngineState::FirstStroke(first) => self.handle_second_stroke(first, key),
            EngineState::PrefixStroke => self.handle_ext_first_stroke(key),
            EngineState::ExtFirstStroke(first) => self.handle_ext_second_stroke(first, key),
        }
    }

    /// キーを消費するかどうかの判定 (TSFのOnTestKeyDown用)
    pub fn will_consume_key(&self, key: char) -> bool {
        match &self.state {
            EngineState::Idle => {
                // Space または有効なキーなら消費
                key == ' ' || self.table.key_to_index(key).is_some()
            }
            // 入力中は全キーを消費
            _ => true,
        }
    }

    /// 状態をリセット
    pub fn reset(&mut self) {
        self.state = EngineState::Idle;
    }

    fn handle_idle(&mut self, key: char) -> EngineOutput {
        if key == ' ' {
            self.state = EngineState::PrefixStroke;
            return EngineOutput::Composing("■".to_string());
        }

        if let Some(idx) = self.table.key_to_index(key) {
            self.state = EngineState::FirstStroke(idx);
            EngineOutput::Consumed
        } else {
            EngineOutput::PassThrough
        }
    }

    fn handle_second_stroke(&mut self, first: usize, key: char) -> EngineOutput {
        self.state = EngineState::Idle;

        // Spaceが第2打鍵の場合、第1打鍵のキー文字を出力
        if key == ' ' {
            if let Some(&ch) = crate::table::QWERTY_KEYS.get(first) {
                return EngineOutput::Commit(ch.to_string());
            }
            return EngineOutput::Clear;
        }

        let Some(second) = self.table.key_to_index(key) else {
            // 無効なキー: 第1打鍵のキー文字を出力してリセット
            return if let Some(&ch) = crate::table::QWERTY_KEYS.get(first) {
                EngineOutput::Commit(ch.to_string())
            } else {
                EngineOutput::Clear
            };
        };

        self.resolve_2stroke(first, second)
    }

    /// 2打鍵テーブルのルックアップと結果処理
    fn resolve_2stroke(&mut self, first: usize, second: usize) -> EngineOutput {
        match self.table.lookup_2stroke(first, second) {
            Some(TableEntry::Char(s)) => {
                let s = s.clone();
                self.history.push(s.clone());
                EngineOutput::Commit(s)
            }
            Some(TableEntry::Special(func)) => {
                self.handle_special_function(func.clone())
            }
            _ => EngineOutput::Clear,
        }
    }

    fn handle_ext_first_stroke(&mut self, key: char) -> EngineOutput {
        if key == ' ' {
            // Space Space = Space出力
            self.state = EngineState::Idle;
            return EngineOutput::Commit(" ".to_string());
        }

        if let Some(idx) = self.table.key_to_index(key) {
            self.state = EngineState::ExtFirstStroke(idx);
            EngineOutput::Composing("■□".to_string())
        } else {
            self.state = EngineState::Idle;
            EngineOutput::Clear
        }
    }

    fn handle_ext_second_stroke(&mut self, first: usize, key: char) -> EngineOutput {
        self.state = EngineState::Idle;

        let Some(second) = self.table.key_to_index(key) else {
            return EngineOutput::Clear;
        };

        match self.table.lookup_3stroke(first, second) {
            Some(TableEntry::Char(s)) => {
                let s = s.clone();
                self.history.push(s.clone());
                EngineOutput::Commit(s)
            }
            Some(TableEntry::Special(func)) => {
                self.handle_special_function(func.clone())
            }
            _ => EngineOutput::Clear,
        }
    }

    /// 特殊機能の処理
    fn handle_special_function(&mut self, func: SpecialFunction) -> EngineOutput {
        match func {
            SpecialFunction::HistoryInput => {
                // 直近の入力文字を再出力
                if let Some(s) = self.history.current().cloned() {
                    EngineOutput::Commit(s)
                } else {
                    EngineOutput::Clear
                }
            }
            SpecialFunction::Cancel => {
                self.state = EngineState::Idle;
                EngineOutput::Clear
            }
            _ => EngineOutput::SpecialAction(func),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_engine() -> Engine {
        let table_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/try.tbl");
        let table = TryCodeTable::load(table_path).expect("failed to load try.tbl");
        Engine::new(table)
    }

    #[test]
    fn test_initial_state() {
        let engine = load_engine();
        assert_eq!(*engine.state(), EngineState::Idle);
    }

    #[test]
    fn test_space_space_produces_space() {
        let mut engine = load_engine();
        let _ = engine.process_key(' '); // → PrefixStroke
        let result = engine.process_key(' '); // → Space出力
        assert_eq!(result, EngineOutput::Commit(" ".to_string()));
        assert_eq!(*engine.state(), EngineState::Idle);
    }

    #[test]
    fn test_passthrough_for_unknown_key() {
        let mut engine = load_engine();
        let result = engine.process_key('`');
        assert_eq!(result, EngineOutput::PassThrough);
    }

    #[test]
    fn test_two_stroke_composing() {
        let mut engine = load_engine();
        let result = engine.process_key('a');
        assert_eq!(result, EngineOutput::Consumed);
        assert!(matches!(engine.state(), EngineState::FirstStroke(_)));
    }

    #[test]
    fn test_key_then_space_outputs_key() {
        let mut engine = load_engine();
        engine.process_key('a'); // FirstStroke
        let result = engine.process_key(' '); // → 'a' 出力
        assert_eq!(result, EngineOutput::Commit("a".to_string()));
    }

    #[test]
    fn test_history_input() {
        let mut engine = load_engine();
        // まず何か文字を入力してヒストリに記録
        engine.process_key('a');
        let result = engine.process_key('k'); // a,k = base[0][10]
        let EngineOutput::Commit(ref ch) = result else {
            panic!("expected Commit, got {:?}", result);
        };

        // ヒストリに記録されている
        assert_eq!(engine.history().current(), Some(ch));
    }
}
