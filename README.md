# rtry - Try-Code Windows IME

Try-Code (T-Code拡張3打鍵入力方式) の Windows IME。Rust + TSF (Text Services Framework) で実装。

## ビルド

```
cargo build --release -p rtry-tsf
```

ビルド成果物: `target\release\rtry_tsf.dll`

## インストール

### 1. テーブルファイルの配置

```cmd
mkdir %APPDATA%\rtry
copy data\try.tbl %APPDATA%\rtry\try.tbl
```

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

### 3. テーブルファイルの削除 (任意)

```cmd
rmdir /s %APPDATA%\rtry
```

## 使い方

Try-Code は2打鍵/3打鍵で直接漢字を入力する方式です。

- **2打鍵入力**: 2つのキーを順に打つと漢字が確定入力されます
- **3打鍵入力**: Space → キー1 → キー2 で拡張テーブルの漢字を入力
- **Space + Space**: スペースを入力
- **キー + Space**: そのキーをそのまま出力 (英字入力)

## 注意事項

- DLLの再ビルド前に `regsvr32 /u` で登録解除するか、IMEを使用中のアプリを閉じてください
- 登録・解除は管理者権限が必要です
