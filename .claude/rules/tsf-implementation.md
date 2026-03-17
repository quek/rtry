---
paths:
  - "crates/rtry-tsf/**/*"
---

# TSF 実装の注意点

## TSF キーイベント
- 一部のアプリ（Windows 11 メモ帳等）は `OnTestKeyDown` を呼ばず `OnKeyDown` を直接呼ぶ
- キーフィルタリングは `OnTestKeyDown` と `OnKeyDown` の**両方**に必要
- `OnKeyDown` の戻り値 `FALSE` でキーはアプリにパススルー

## CUAS 環境（Emacs 等 IMM32 アプリ）

CUAS 互換レイヤーのテキストストアは書き込み専用（`ShiftStart`/`GetText` が 0 を返す）。

### SharedPostBuf
- 確定テキストを最大 10 文字保持する内部バッファ（TSF 読み取り失敗時のフォールバック）
- IME オン後に入力した文字のみ蓄積。既存テキストに対する操作は不可

### VKBackBasedDeleter（交ぜ書き確定フロー）
- tsf-tutcode/Mozc 由来のパターン
- N+1 個の VK_BACK を SendInput で送信
- 最初の N 個: `OnTestKeyDown` で FALSE を返しアプリに渡す（読みを削除）
- 番兵（N+1 個目）: IME が消費して do_commit を実行
- **重要**: `RequestEditSession(TF_ES_ASYNCDONTCARE)` は同期実行される場合がある。SendInput 後に直接 do_commit を呼んではならない

## IME オン/オフ
- コンパートメント方式（`GUID_COMPARTMENT_KEYBOARD_OPENCLOSE`）は使わない
- PreservedKey 方式も使わない（システムとの競合リスク）
- `Arc<AtomicBool>` の内部フラグで管理し、`OnKeyDown` で Alt+`/VK_KANJI を直接処理
- 言語バーボタンと `is_open` を共有（「漢」⇔「A」切り替え）

## カーソル位置
- テキスト確定後は `Collapse(TF_ANCHOR_END)` + `SetSelection` でカーソルを末尾に移動

## AppContainer 対応
- SearchHost.exe 等は `%APPDATA%` にアクセスできない
- DLL・データは `C:\Program Files\rtry\` に配置（AppContainer から読み取り可能）
- 設定ファイル検索順: DLL と同じディレクトリ → `%APPDATA%\rtry\`（フォールバック）
- ACL 対応（CorvusSKK 方式）: rtry-config が設定保存時に `ALL_APP_PACKAGES`（AC）読み取り権限を SDDL で設定

## インストール構成
- DLL・データ: `C:\Program Files\rtry\`（rtry_tsf.dll, try.tbl, mazegaki.dic, config.json, debug.log）
- `install.bat` で `%APPDATA%\rtry\config.json` を DLL ディレクトリにもコピー
- プロファイル登録: `ITfInputProcessorProfileMgr::RegisterProfile`（`bEnabledByDefault: true`）
- カテゴリ: `GUID_TFCAT_TIP_KEYBOARD`, `GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT`, `GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT`
