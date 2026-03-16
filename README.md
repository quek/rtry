# rtry - Try-Code Windows IME

Try-Code (T-Code拡張3打鍵入力方式) の Windows IME。Rust + TSF (Text Services Framework) で実装。

## ビルド

```
cargo build --release -p rtry-tsf
```

ビルド成果物: `target\release\rtry_tsf.dll`

## インストール

### 1. データファイルの配置

```cmd
mkdir %APPDATA%\rtry
copy data\try.tbl %APPDATA%\rtry\try.tbl
copy data\mazegaki.dic %APPDATA%\rtry\mazegaki.dic
```

交ぜ書き辞書は同梱のサンプル（約300エントリ）の他、[SKK-JISYO.L](https://github.com/skk-dev/dict) を変換して使用できます。

### 2. DLL登録 (管理者権限が必要)

管理者権限でコマンドプロンプトを開き:

```cmd
regsvr32 F:\dev\rtry\target\release\rtry_tsf.dll
```

成功すると「DllRegisterServer は成功しました」と表示されます。

### 3. IMEの有効化

1. 設定 → 時刻と言語 → 言語と地域
2. 日本語 → 言語オプション → キーボードの追加
3. 「Try-Code」を選択

### 4. IMEの切り替え

`Win + Space` でIMEを切り替えます。タスクバーの入力インジケータからも選択可能。

## アンインストール

### 1. IMEの無効化

1. 設定 → 時刻と言語 → 言語と地域
2. 日本語 → 言語オプション
3. 「Try-Code」の横の「...」→ 削除

### 2. DLL登録解除 (管理者権限が必要)

```cmd
regsvr32 /u F:\dev\rtry\target\release\rtry_tsf.dll
```

### 3. データファイルの削除 (任意)

```cmd
rmdir /s %APPDATA%\rtry
```

## 使い方

Try-Code は2打鍵/3打鍵で直接漢字を入力する方式です。

- **2打鍵入力**: 2つのキーを順に打つと漢字が確定入力されます
- **3打鍵入力**: Space → キー1 → キー2 で拡張テーブルの漢字を入力
- **Space + Space**: スペースを入力
- **キー + Space**: そのキーをそのまま出力 (英字入力)
- **IME オン/オフ**: Alt+` または 半角/全角キー

### 交ぜ書き変換

T-Codeストロークで直接入力できない漢字を、読み（ひらがな）から変換します。

1. ひらがなを入力（例: 「きしゃ」）
2. `fj` を押す → カーソル前のテキストから最長一致で辞書検索
3. 候補ウィンドウが表示される
4. キー操作:
   - **Space**: 次の候補
   - **1-9**: 番号で候補を選択して確定
   - **Enter**: 現在の候補で確定
   - **Escape**: キャンセル（元の読みに戻す）

### ストロークヘルプ

カーソル直前の文字のストローク（打鍵手順）を表示します。

- `55`（キー5を2回）で起動
- `44`（キー4を2回）で直前のストロークヘルプを再表示

## 注意事項

- DLLの再ビルド前に `regsvr32 /u` で登録解除するか、IMEを使用中のアプリを閉じてください
- 登録・解除は管理者権限が必要です
