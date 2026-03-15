# rtry - Try-Code Windows IME

T-Code を拡張した Try-Code の Windows IME。Rust で TSF (Text Services Framework) ベースで実装。

## プロジェクト構成

- `crates/rtry-core/` - コアエンジン（テーブルパーサー、入力状態マシン、交ぜ書き辞書）
- `crates/rtry-tsf/` - Windows TSF DLL（COM インターフェース、キーハンドラ、EditSession、候補ウィンドウ）
- `crates/rtry-config/` - 設定 GUI（未実装）
- `data/try.tbl` - try-code 変換テーブル（miau氏のgistから取得）
- `data/mazegaki.dic` - 交ぜ書き辞書サンプル（SKK辞書形式）

## ユーザー環境

- US キーボード（物理的に P の下のキーが `-`）
- VK_OEM_MINUS (0xBD) → `;` としてエンジンに渡す（T-Code の40キー配列の `;` 位置に対応）
- IME オン/オフトグルは Alt+` を使用

## ビルド・テスト手順

```
cargo build --release -p rtry-tsf
install.bat   # regsvr32 で DLL 登録
uninstall.bat # regsvr32 /u で DLL 登録解除
```

### DLL ロック問題
- ビルド前に `uninstall.bat` を実行し、IME を使用中のアプリを全て閉じる
- `tasklist /m rtry_tsf.dll` でロックしているプロセスを特定する
- それでもロックが残る場合は `rm -f target/release/rtry_tsf.dll` してからビルド
- **別のパスにビルドするな**。ロックしているプロセスを特定して解消すること

## 開発のルール

### コーディング原則
- KISS: 最もシンプルなアプローチを最初に試す
- DRY: 重複を排除する
- edition 2024, 最新の crate バージョンを使用
- モダンな Rust イディオム
- コードを書いたら都度レビュー、リファクタリングを行う

### 調査・デバッグ手順
- **推測でコードを書くな**。まずコードを読んで原因を追ってから修正する
- **デバッグは上流から下流へ**: まず関数が呼ばれているか → 引数は正しいか → ロジックは正しいか の順で確認する
- **前提を検証してから実装する**: 「この関数が呼ばれるはず」等の前提はログで検証してから次に進む
- ログ追加の前に、コードのフローを追って原因を特定する
- 「可能性がある」ではなく、確実に原因を特定してから修正する
- **実行するスクリプト・コマンドは中身を確認してから指示する**
- windows crate の API は `~/.cargo/registry/src/` 内のソースを grep して確認する
- **使用するライブラリのガイド・リファレンスマニュアルを調査してから実装する**
  - windows crate: https://microsoft.github.io/windows-docs-rs/
  - TSF (Text Services Framework): https://learn.microsoft.com/en-us/windows/win32/tsf/text-services-framework
  - COM プログラミング: https://learn.microsoft.com/en-us/windows/win32/com/component-object-model--com--portal
- **類似プロダクトのソースコードを参考にする**
  - Microsoft TSF サンプル IME: https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/IME
  - tsf-tutcode (Rust TSF IME): https://github.com/deton/tsf-tutcode
  - CorvusSKK (C++ TSF IME): https://github.com/corvusskk/corvusskk
  - 既存の T-Code IME 実装（tc2, tcode-mode 等）の設計を参考にする

## rtry-tsf アーキテクチャ

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

## Windows TSF 実装の注意点

### TSF キーイベント
- 一部のアプリ（Windows 11 メモ帳等）は `OnTestKeyDown` を呼ばず `OnKeyDown` を直接呼ぶ
- そのため、キーフィルタリング（修飾キーチェック等）は `OnTestKeyDown` と `OnKeyDown` の両方に必要
- `OnKeyDown` の戻り値 `FALSE` でキーはアプリにパススルーされる

### CUAS環境（Emacs等）向け postbuf / VKBackBasedDeleter
- CUAS互換レイヤーのテキストストアは書き込み専用（ShiftStart/GetText が 0 を返す）
- `SharedPostBuf`: 確定テキストを最大10文字保持する内部バッファ（TSF読み取り失敗時のフォールバック）
- `PendingReplace`: VKBackBasedDeleterパターン（tsf-tutcode/Mozc由来）の状態
- 交ぜ書き確定フロー: N+1個のVK_BACKをSendInputで送信 → 最初のN個はOnTestKeyDownでFALSEを返しアプリに渡す → 番兵をIMEが消費してdo_commitを実行
- `RequestEditSession(TF_ES_ASYNCDONTCARE)` は同期実行される場合があり、SendInputキューより先に処理されるため、SendInput後に直接do_commitを呼んではならない

### IME オン/オフの実装
- `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE` コンパートメント方式は使わない
- `PreservedKey` 方式も使わない（システムとの競合リスク）
- `RefCell<bool>` の内部フラグで管理し、`OnKeyDown` で Alt+`/VK_KANJI を直接処理

### カーソル位置
- テキスト確定後は `Collapse(TF_ANCHOR_END)` + `SetSelection` でカーソルを末尾に移動
- IME の常識：変換後の文字はカーソルの左側に挿入される

### Edition 2024 の罠
- `ManuallyDrop` union フィールドへの書き込みに `(*field)` が必要
- VARIANT の読み書きは信頼性が低い。内部フラグ（RefCell<bool>）の方が確実
- `#[unsafe(no_mangle)]` が必要
- `unsafe fn` の本体内でも `unsafe {}` ブロックが必要

### windows crate 0.62 の API
- COM メソッドの引数型は `Ref<'_, T>`（`Option<&T>` ではない）
- `RequestEditSession` は 3 引数（4 ではない）
- `ITfComposition::EndComposition` は `ec: u32` 引数が必要
- `TF_ANCHOR_END` が正しい定数名（`TfAnchor_TF_ANCHOR_END` ではない）

## テーブル形式 (try.tbl)
- 40個の深さ2フラットセクション = base table（2打鍵、40×40）
- 1個の深さ2ネストセクション（40個の深さ3サブブロック）= ext table（3打鍵、Space接頭辞）
- `@m` = 交ぜ書き変換, `@b` = 部首合成, `@q` = ヒストリ入力, `@!` = キャンセル

## 実装済み機能
- 2打鍵/3打鍵（Space接頭辞）による直接漢字入力
- 交ぜ書き変換（`fj` トリガー、最長一致、SKK辞書形式）
- ストロークヘルプ（`55` でカーソル前文字の打鍵手順表示）
- 候補ウィンドウ（Win32ポップアップ、番号選択対応）
- IME オン/オフトグル（Alt+` / 半角全角）
- 修飾キー（Ctrl/Shift/Alt）付きキーのパススルー（C-c, C-v 等）

## 既知の制限

### CUAS環境（Emacs等）での制限
Emacs等のIMM32ベースアプリでは、CUAS互換レイヤーのテキストストアが書き込み専用のため、
TSFの`GetText`で既存テキストを読み取れない（`ShiftStart`=0, `GetText`=0文字）。

- **対策**: 確定済みテキストを内部バッファ（postbuf、最大10文字）に記録し、TSF読み取り失敗時にフォールバック使用（tsf-tutcode方式）
- **ストロークヘルプ**: postbuf末尾1文字で逆引き
- **交ぜ書き変換**: postbuf内容で最長一致検索。候補選択中はドキュメント非更新。確定時はVKBackBasedDeleterパターン（tsf-tutcode/Mozc由来）でN+1個のVK_BACKをSendInputで送信し、最初のN個はアプリに渡して読みを削除、最後の番兵をIMEが消費してTSF commitを実行。TSFコールバックは同一スレッドで直列処理されるため順序保証される
- **制約**: IMEオン後に入力した文字のみpostbufに蓄積されるため、既存テキストに対する操作は不可

## 未実装機能
- 後置型交ぜ書き変換（18-98）
- 部首合成（`@b`）
- ヒストリ入力（`@q`）
- 設定 GUI（rtry-config）
- インストーラ
