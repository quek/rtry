# rtry - Try-Code Windows IME

T-Code を拡張した Try-Code の Windows IME。Rust + TSF (Text Services Framework) で実装。

## ビルド・インストール

```
cargo build --release -p rtry-tsf
uninstall.bat   # DLL 登録解除・削除（管理者権限）
install.bat     # DLL・データを C:\Program Files\rtry\ にコピー＆登録（管理者権限）
```

### DLL ロック問題
- ビルド前に `uninstall.bat` を実行し、IME を使用中のアプリを全て閉じる
- `tasklist /m rtry_tsf.dll` でロックしているプロセスを特定
- **別のパスにビルドするな**。ロックしているプロセスを特定して解消すること

## 開発ルール

### コミット
- コミットメッセージは日本語で書く

### コーディング
- KISS・DRY
- edition 2024、最新の crate バージョン
- コードを書いたら都度レビュー・リファクタリング

### 調査・デバッグ
- **推測でコードを書くな**。まずコードを読んで原因を追ってから修正する
- **推測で発言するな**。「〜のはず」「〜とは無関係」等の断定はログやコードで検証してから述べる
- **安易な解決策を採用するな**。類似プロダクトの実装やベストプラクティスを調べてから実装する
- **デバッグは上流から下流へ**: 関数が呼ばれているか → 引数は正しいか → ロジックは正しいか
- **前提を検証してから実装する**: 「呼ばれるはず」等の前提はログで検証してから次に進む
- **デバッグログは自分で読め**: `$TEMP/rtry_debug.log` を確認してから報告する。ユーザーに確認を委ねるな
- 「可能性がある」ではなく、確実に原因を特定してから修正する
- windows crate の API は `~/.cargo/registry/src/` 内のソースを grep して確認する

## 重要な罠

### Windows パス権限
- `C:\Program Files\` は管理者権限でしか書き込めない。ユーザープロセスが書き込むファイル（設定、ログ等）は `%APPDATA%` や `%TEMP%` に置く
- 修正案を出す前にファイルの書き込み先が適切か（権限の制約）を検証する

### Edition 2024
- `ManuallyDrop` union フィールドへの書き込みに `(*field)` が必要
- `#[unsafe(no_mangle)]` が必要
- `unsafe fn` 本体内でも `unsafe {}` ブロックが必要

### windows crate 0.62
- COM メソッドの引数型は `Ref<'_, T>`（`Option<&T>` ではない）
- `RequestEditSession` は 3 引数（4 ではない）
- `ITfComposition::EndComposition` は `ec: u32` 引数が必要
- `TF_ANCHOR_END` が正しい定数名（`TfAnchor_TF_ANCHOR_END` ではない）
- VARIANT の読み書きは信頼性が低い。内部フラグ（`RefCell<bool>`）の方が確実

## 交ぜ書き変換
- `data/mazegaki.dic`: tc2辞書 + SKK-JISYO.L をマージした辞書（184,013件）
- `scripts/merge_mazegaki.py`: 再マージ用スクリプト
- 活用語対応: `―`マーカー付きエントリ（例: `あきらめ― /諦め/`）に対応
  - 末尾1〜4文字のひらがなを活用語尾として切り離し、語幹+―で辞書検索
  - 活用語が完全一致に勝つには、語幹文字数 >= 完全一致の文字数が必要
- 読み長さ変更: 候補表示中にカーソル左右で変換対象文字数を変更可能
  - ←: 読みを伸ばす（次に長いマッチ）/ →: 読みを縮める（次に短いマッチ）
  - `MazegakiResizeEditSession` でコンポジション終了→再開を1 EditSessionで実行

## 未実装機能
- 後置型交ぜ書き変換（18-98）
- 部首合成（`@b`）
- ヒストリ入力（`@q`）
- インストーラ
