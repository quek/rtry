# 調査対象プロジェクト

| プロジェクト | 言語 | 特徴 | クローン先 | URL |
|---|---|---|---|---|
| tsf-tutcode | C++ | T-Code 系 TSF IME、rtry と最も近い設計。**最優先** | /tmp/tsf-tutcode | https://github.com/deton/tsf-tutcode |
| CorvusSKK | C++ | 高品質な TSF IME、SKK 方式 | /tmp/corvusskk | https://github.com/corvusskk/corvusskk |
| Microsoft SampleIME | C++ | Microsoft 公式の TSF サンプル | /tmp/sample-ime | https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/IME |

全プロジェクトを調査する必要はない。機能に最も関連するものを優先する。

# API リファレンス・ガイド

| ドキュメント | URL |
|---|---|
| TSF (Text Services Framework) | https://learn.microsoft.com/en-us/windows/win32/tsf/text-services-framework |
| COM プログラミング | https://learn.microsoft.com/en-us/windows/win32/com/component-object-model--com--portal |
| windows crate (Rust) | https://microsoft.github.io/windows-docs-rs/ |
| Win32 API | https://learn.microsoft.com/en-us/windows/win32/api/ |

# TSF 機能と API の対応例

| 機能 | 主な API / インターフェース |
|---|---|
| カーソル位置の取得 | `ITfContextView::GetTextExt` |
| 候補ウィンドウ | `ITfCandidateListUIElement` or Win32 popup |
| 入力モード表示 | `ITfLangBarItemButton` or indicator window |
| テキスト属性 | `ITfDisplayAttributeProvider` |
| テキスト編集 | `ITfEditSession`, `ITfRange` |
| キーイベント | `ITfKeyEventSink` |
| コンパートメント | `ITfCompartment`, `ITfCompartmentEventSink` |
