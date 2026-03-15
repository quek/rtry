---
name: research-similar-impl
description: |
  類似プロダクトのソースコードを調査してから実装する。新機能の実装やバグ修正の前に、
  類似のオープンソースプロジェクトがどのようにその機能を実装しているかを調査し、
  API の使い方、設計パターン、互換性の注意点をレポートする。
  CLAUDE.md に「類似プロダクトのソースコードを参考にする」とあるように、
  推測でコードを書かず、まず調査してから実装に入ることが重要。
  ユーザーが新機能の実装を依頼したとき、バグの修正を依頼したとき、
  Windows API や COM インターフェースの使い方が不明なとき、
  「実装して」「追加して」「修正して」「対応して」等の指示があったときに発動する。
  実装の計画段階（EnterPlanMode）で使うのが最適。
---

# 類似プロダクト調査スキル

## 目的

CLAUDE.md の開発ルールに「推測でコードを書くな」「類似プロダクトのソースコードを参考にする」とある。
このスキルは、実装を始める前に類似プロジェクトを体系的に調査し、
確実な根拠に基づいた実装方針を立てるためのもの。

## 調査対象プロジェクト

対象の機能に応じて、以下のプロジェクトのソースコードを調査する:

| プロジェクト | 言語 | 特徴 | URL |
|---|---|---|---|
| tsf-tutcode | C++ | T-Code 系 TSF IME、rtry と最も近い設計 | https://github.com/deton/tsf-tutcode |
| CorvusSKK | C++ | 高品質な TSF IME、SKK 方式 | https://github.com/corvusskk/corvusskk |
| Microsoft SampleIME | C++ | Microsoft 公式の TSF サンプル | https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/IME |

## 調査手順

### ステップ 1: 機能の特定

ユーザーの要求から、実装対象の TSF 機能・Windows API を特定する。例:
- カーソル位置の取得 → `ITfContextView::GetTextExt`
- 候補ウィンドウ → `ITfCandidateListUIElement` or Win32 popup
- 入力モード表示 → `ITfLangBarItemButton` or indicator window
- テキスト属性 → `ITfDisplayAttributeProvider`

### ステップ 2: ソースコード調査

Agent ツール（subagent_type: general-purpose）を使って、各プロジェクトのリポジトリから
該当機能の実装を検索・読解する。並列に複数のエージェントを起動して効率化する。

調査のポイント:
1. **API の実際の呼び出しパターン** — 関数シグネチャ、引数の型、戻り値の扱い
2. **設計パターン** — クラス/構造体の構成、状態管理、COM インターフェースの実装方法
3. **エラーハンドリング** — 失敗時のフォールバック、HRESULT のチェック方法
4. **CUAS 互換性** — CUAS（Emacs 等の IMM32 アプリ）での制限と回避策
5. **スレッドモデル** — STA/MTA、EditSession の同期/非同期

### ステップ 3: windows crate の API 確認

rtry は Rust の `windows` crate を使っているため、C++ の API と Rust のバインディングで
シグネチャが異なる場合がある。必ず `~/.cargo/registry/src/` 内のソースを grep して
実際の Rust シグネチャを確認する。

確認すべき点:
- COM メソッドの引数型（`Ref<'_, T>` vs `Option<&T>` vs 生ポインタ）
- `Result<T>` のラッピング
- `ManuallyDrop` や `VARIANT` の扱い
- 定数名の違い（例: `TF_ANCHOR_END` vs `TfAnchor_TF_ANCHOR_END`）

### ステップ 4: レポート出力

以下の形式で調査結果をまとめる（日本語で）:

```
## 調査結果: [機能名]

### 各プロジェクトの実装

#### tsf-tutcode
- ファイル: ...
- パターン: ...
- API 呼び出し: ...

#### CorvusSKK / SampleIME
- ...

### 推奨アプローチ
- rtry での実装方針
- 採用する設計パターンとその理由

### windows crate の API シグネチャ
- 確認した関数とそのシグネチャ

### CUAS/互換性の注意点
- CUAS 環境での制限と対策
- エッジケースの扱い

### 参考コード
- 具体的なコード例（C++ → Rust への変換ポイント）
```

## 注意事項

- 調査は実装の**前**に行う。コードを書き始めてから調査するのは非効率
- 3 つのプロジェクト全てを調査する必要はない。機能に最も関連するものを優先する
- tsf-tutcode は rtry と同じ T-Code 系で設計が最も近いため、最優先で参照する
- windows crate の API 確認は省略しない。C++ と Rust でシグネチャが異なることが多い
- **このスキルは調査のみを行う。ファイルの編集・作成・ビルド・インストール等、環境を変更する操作は一切行わない**
