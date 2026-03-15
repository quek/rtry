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

### 調査・デバッグ手順
- **推測でコードを書くな**。まずコードを読んで原因を追ってから修正する
- ログ追加の前に、コードのフローを追って原因を特定する
- 「可能性がある」ではなく、確実に原因を特定してから修正する
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

### Windows TSF 実装の注意点
- edition 2024 では `ManuallyDrop` union フィールドへの書き込みに `(*field)` が必要
- VARIANT の読み書きは信頼性が低い。内部フラグ（RefCell<bool>）の方が確実
- `#[unsafe(no_mangle)]` が必要（edition 2024）
- `unsafe fn` の本体内でも `unsafe {}` ブロックが必要
- windows 0.62 の COM メソッドの引数型は `Ref<'_, T>`（`Option<&T>` ではない）
- `RequestEditSession` は 3 引数（4 ではない）
- `ITfComposition::EndComposition` は `ec: u32` 引数が必要

### IME オン/オフの実装
- `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE` コンパートメント方式は使わない
- `PreservedKey` 方式も使わない（システムとの競合リスク）
- `RefCell<bool>` の内部フラグで管理し、`OnKeyDown` で Alt+`/VK_KANJI を直接処理

### カーソル位置
- テキスト確定後は `Collapse(TF_ANCHOR_END)` + `SetSelection` でカーソルを末尾に移動
- IME の常識：変換後の文字はカーソルの左側に挿入される

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

## 既知の制限

### CUAS環境（Emacs等）での制限
Emacs等のIMM32ベースアプリでは、CUAS互換レイヤーのテキストストアが書き込み専用のため、
TSFの`GetText`で既存テキストを読み取れない（`ShiftStart`=0, `GetText`=0文字）。

- **対策**: 確定済みテキストを内部バッファ（postbuf、最大10文字）に記録し、TSF読み取り失敗時にフォールバック使用（tsf-tutcode方式）
- **ストロークヘルプ**: postbuf末尾1文字で逆引き
- **交ぜ書き変換**: postbuf内容で最長一致検索。候補選択中はドキュメント非更新、確定時にSendInputでバックスペース送信後に候補テキストを挿入
- **制約**: IMEオン後に入力した文字のみpostbufに蓄積されるため、既存テキストに対する操作は不可

## 未実装機能
- 後置型交ぜ書き変換（18-98）
- 部首合成（`@b`）
- ヒストリ入力（`@q`）
- 設定 GUI（rtry-config）
- インストーラ
