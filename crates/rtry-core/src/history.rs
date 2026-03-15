//! ヒストリ入力
//!
//! 直近の入力文字を記憶し、再入力する機能。
//! try-codeでは `66` (key 6 を2回) や `:` でヒストリ入力を呼び出す。

use std::collections::VecDeque;

/// ヒストリ入力マネージャ
pub struct HistoryManager {
    /// 入力履歴 (最新が先頭)
    history: VecDeque<String>,
    /// 最大履歴数
    max_size: usize,
    /// 現在のヒストリ選択位置
    cursor: usize,
}

impl HistoryManager {
    pub fn new(max_size: usize) -> Self {
        HistoryManager {
            history: VecDeque::new(),
            max_size,
            cursor: 0,
        }
    }

    /// 文字列を履歴に追加
    pub fn push(&mut self, text: String) {
        // 重複を除去
        self.history.retain(|h| h != &text);
        self.history.push_front(text);
        if self.history.len() > self.max_size {
            self.history.pop_back();
        }
        self.cursor = 0;
    }

    /// 現在位置のヒストリを取得
    pub fn current(&self) -> Option<&String> {
        self.history.get(self.cursor)
    }

    /// 次のヒストリに移動
    pub fn next(&mut self) -> Option<&String> {
        if self.cursor + 1 < self.history.len() {
            self.cursor += 1;
        }
        self.current()
    }

    /// 前のヒストリに移動
    pub fn prev(&mut self) -> Option<&String> {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        self.current()
    }

    /// カーソルをリセット
    pub fn reset_cursor(&mut self) {
        self.cursor = 0;
    }

    /// 履歴一覧を取得
    pub fn entries(&self) -> &VecDeque<String> {
        &self.history
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_current() {
        let mut hm = HistoryManager::new(10);
        hm.push("あ".to_string());
        hm.push("い".to_string());
        assert_eq!(hm.current(), Some(&"い".to_string()));
    }

    #[test]
    fn test_navigation() {
        let mut hm = HistoryManager::new(10);
        hm.push("あ".to_string());
        hm.push("い".to_string());
        hm.push("う".to_string());

        assert_eq!(hm.current(), Some(&"う".to_string()));
        assert_eq!(hm.next(), Some(&"い".to_string()));
        assert_eq!(hm.next(), Some(&"あ".to_string()));
        assert_eq!(hm.prev(), Some(&"い".to_string()));
    }

    #[test]
    fn test_dedup() {
        let mut hm = HistoryManager::new(10);
        hm.push("あ".to_string());
        hm.push("い".to_string());
        hm.push("あ".to_string()); // 重複

        assert_eq!(hm.entries().len(), 2);
        assert_eq!(hm.current(), Some(&"あ".to_string()));
    }

    #[test]
    fn test_max_size() {
        let mut hm = HistoryManager::new(3);
        hm.push("あ".to_string());
        hm.push("い".to_string());
        hm.push("う".to_string());
        hm.push("え".to_string());

        assert_eq!(hm.entries().len(), 3);
    }
}
