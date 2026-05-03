---
paths:
  - "crates/rtry-tsf/**/*"
---

# TSF 実装の注意点

## TSF キーイベント
- 一部のアプリ（Windows 11 メモ帳、cmd.exe 等）は `OnTestKeyDown` を呼ばず `OnKeyDown` を直接呼ぶ
- キーフィルタリングは `OnTestKeyDown` と `OnKeyDown` の**両方**に必要
- **両メソッドのキー処理ロジックは必ず同一にする**。片方だけ修正するともう片方の呼び出しパターンのアプリで壊れる
- `OnKeyDown` の戻り値 `FALSE` でキーはアプリにパススルー

## CUAS 環境（Emacs 等 IMM32 アプリ）

CUAS 互換レイヤーのテキストストアは書き込み専用（`ShiftStart`/`GetText` が 0 を返す）。

### SharedPostBuf
- 確定テキストを最大 10 文字保持する内部バッファ（TSF 読み取り失敗時のフォールバック）
- IME オン後に入力した文字のみ蓄積。既存テキストに対する操作は不可
- BS パススルー時は `postbuf_remove_tail(1)` で同期する（OnTestKeyDown / OnKeyDown 両方）
- cmd.exe も CUAS 環境（`GetText` が 0 を返す）であり PostBuf パスを使う

### VKBackBasedDeleter（交ぜ書き確定フロー）
- tsf-tutcode/Mozc 由来のパターン
- N+1 個の VK_BACK を SendInput で送信
- 最初の N 個: `remaining_bs` をデクリメントし FALSE を返してアプリに渡す（読みを削除）
- 番兵（N+1 個目）: IME が消費して do_commit を実行
- **`remaining_bs` のチェックは `OnTestKeyDown` と `OnKeyDown` の両方に必要**（cmd 等は OnKeyDown のみ呼ぶため）
- **重要**: `RequestEditSession(TF_ES_ASYNCDONTCARE)` は同期実行される場合がある。SendInput 後に直接 do_commit を呼んではならない

## IME オン/オフ
- コンパートメント方式（`GUID_COMPARTMENT_KEYBOARD_OPENCLOSE`）は使わない
- PreservedKey 方式も使わない（システムとの競合リスク）
- `Arc<AtomicBool>` の内部フラグで管理し、`OnKeyDown` で Alt+`/VK_KANJI を直接処理
- 言語バーボタンと `is_open` を共有（「漢」⇔「A」切り替え）

## カーソル位置
- テキスト確定後は `Collapse(TF_ANCHOR_END)` + `SetSelection` でカーソルを末尾に移動

## ファイルパスと権限
- `C:\Program Files\rtry\`: 読み取り専用（管理者権限でのみ書き込み可）。DLL・try.tbl・mazegaki.dic の配置先
- `%ProgramData%\rtry\`: 設定ファイル (config.json) の保存先。`install.bat` で作成し `icacls /grant *S-1-5-32-545:(OI)(CI)M` で Users に変更権限を付与しているため、rtry-config が管理者権限なしで保存できる
- `%APPDATA%\rtry\`: **使わない**（MSIX のファイル仮想化対象なので使用禁止）。`install.bat` が一度だけここから ProgramData に config.json をマイグレートする
- **原則**: 共有設定は `%ProgramData%` に置く。Program Files にはバイナリとデフォルトデータのみ。`%APPDATA%` は MSIX 環境で AppContainer ごとに別実体になるため使用しない

## AppContainer / MSIX パッケージ対応
- **核心問題**: Microsoft Store 経由の MSIX パッケージ (Claude Desktop App `Claude_pzs8sxrjxfjjc` 等) は `%APPDATA%` がパッケージ専用フォルダ (`%LOCALAPPDATA%\Packages\<PFN>\LocalCache\Roaming\`) にコピーオンライトで仮想化される。`SHGetKnownFolderPath` の `KF_FLAG_NO_PACKAGE_REDIRECTION` フラグはパス文字列の取得には効くが、その後の `std::fs::read_to_string` 等のファイルアクセスでは別機構で再仮想化されるため不十分
- **解決策**: 設定ファイルを `%ProgramData%\rtry\config.json` に置く。ProgramData は Microsoft 仕様で MSIX/UWP のファイル仮想化対象外で、AppContainer 含む全プロセスから同一実体として見える
- DLL・データは `C:\Program Files\rtry\` に配置（読み取り専用、AppContainer から読み取り可能）
- 設定ファイル検索順: `%ProgramData%\rtry\config.json` → DLL と同じディレクトリ (`%ProgramFiles%\rtry\config.json`、フォールバック)
- 念のため rtry-config は `%ProgramData%\rtry\config.json` 保存時に SDDL `(A;;FR;;;AC)` を付与（DACL は ProgramData 既定権限で十分だが、CorvusSKK 流の保険）

## インストール構成
- DLL・データ: `C:\Program Files\rtry\`（rtry_tsf.dll, try.tbl, mazegaki.dic, rtry-config.exe）
- 設定ファイル: `C:\ProgramData\rtry\config.json`
- デバッグログ: `%TEMP%\rtry_debug.log`（Mozc/glog 流、AppContainer からも書き込み可）
- `install.bat` が `%ProgramData%\rtry` を作成 → Users にフルコントロール付与 → 旧 `%APPDATA%\rtry\config.json` があれば一度だけマイグレート
- プロファイル登録: `ITfInputProcessorProfileMgr::RegisterProfile`（`bEnabledByDefault: true`）
- カテゴリ: `GUID_TFCAT_TIP_KEYBOARD`, `GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT`, `GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT`
