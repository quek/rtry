# rtry-tsf

Try-Code Windows IME の TSF (Text Services Framework) 実装。

## アーキテクチャ

### COM インターフェース
- `ITfTextInputProcessor` - Activate/Deactivate（text_service.rs）
- `ITfKeyEventSink` - OnTestKeyDown/OnKeyDown/OnPreservedKey（key_handler.rs）
- `ITfCompositionSink` - OnCompositionTerminated（text_service.rs）
- `ITfEditSession` - DoEditSession の 6 種類（edit_session.rs）
- `IClassFactory` - CreateInstance（class_factory.rs）

### ファイル構成
- `lib.rs` - DllMain, DllGetClassObject 等の DLL エントリポイント、GUID 定義
- `text_service.rs` - TryCodeTextService 本体（Activate/Deactivate、エンジン初期化、SharedPostBuf、PendingReplace）
- `key_handler.rs` - キーイベント処理、IME オン/オフトグル、vk_to_char 変換、交ぜ書きキー処理、VKBackBasedDeleter
- `edit_session.rs` - Commit, Composing, EndComposition, CharHelp, MazegakiStart, MazegakiUpdate の各 EditSession
- `composition.rs` - SharedComposition（Arc<Mutex<Option<ITfComposition>>>）
- `candidate_window.rs` - 交ぜ書き候補ウィンドウ（Win32ポップアップ、番号付き候補表示）
- `stroke_help.rs` - ストロークヘルプ表示、カーソル位置取得（get_caret_screen_pos）
- `language_bar.rs` - 言語バーボタン
- `register.rs` - regsvr32 用の COM/TSF 登録処理
- `class_factory.rs` - IClassFactory 実装

## Coding Principles

### ベストプラクティスを追求する
- 最新のベストプラクティスでの実装を行なう
- edition や各 crate は最新のバージョンを使用する
- モダンな Rust イディオムを積極的に採用する

### KISS (Keep It Simple, Stupid)

### DRY (Don't Repeat Yourself)

### 整合性の維持

### 改善
コードを書いたら都度レビュー、リファクタリングを行なう。
CLAUDE.md ファイルを継続的に改善する。

## 開発上の注意

### 調査・デバッグ
- 推測でコードを書くな。コードを追って原因を特定してから修正する
- ログ追加の前にコードフローの追跡を完了させる
- windows crate の API は `~/.cargo/registry/src/` 内のソースを grep して確認

### DLL ロック
- ビルド前に uninstall.bat を実行
- `tasklist /m rtry_tsf.dll` でプロセスを特定
- 別のパスにビルドするな

### Edition 2024 の罠
- `#[unsafe(no_mangle)]` が必要
- `ManuallyDrop` union フィールドへの書き込みに `(*field)` が必要
- `unsafe fn` の本体内でも `unsafe {}` ブロックが必要

### CUAS環境（Emacs等）向け postbuf / VKBackBasedDeleter
- CUAS互換レイヤーのテキストストアは書き込み専用（ShiftStart/GetText が 0 を返す）
- `SharedPostBuf`: 確定テキストを最大10文字保持する内部バッファ（TSF読み取り失敗時のフォールバック）
- `PendingReplace`: VKBackBasedDeleterパターン（tsf-tutcode/Mozc由来）の状態
- 交ぜ書き確定フロー: N+1個のVK_BACKをSendInputで送信 → 最初のN個はOnTestKeyDownでFALSEを返しアプリに渡す → 番兵をIMEが消費してdo_commitを実行
- `RequestEditSession(TF_ES_ASYNCDONTCARE)` は同期実行される場合があり、SendInputキューより先に処理されるため、SendInput後に直接do_commitを呼んではならない

### windows crate 0.62 の API
- COM メソッドの引数は `Ref<'_, T>`（`Option<&T>` ではない）
- `RequestEditSession` は 3 引数
- `ITfComposition::EndComposition(ec)` は ec 引数が必要
- `TF_ANCHOR_END` が正しい定数名（`TfAnchor_TF_ANCHOR_END` ではない）
