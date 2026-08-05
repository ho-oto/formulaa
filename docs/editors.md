# エディタ統合 (VSCode / Obsidian / Zed)

目標体験: 「基本はテキストとして編集し、数式になっている部分だけは
TUI エディタ相当の構造編集ができる」。数式は Markdown の
` ```math ` フェンスブロックに AA 形式で埋め込む(AA は正準形式なので、
いつでも再パースして構造編集・LaTeX 変換ができる)。

## リポジトリ構成

拡張はそれぞれ**別リポジトリ**にある。このリポジトリは lib と CLI
だけを持ち、拡張のビルド事情(npm・wasm-pack・vsix・vault へのコピー)を
抱え込まない:

| | リポジトリ | 形態 |
|---|---|---|
| VSCode | [ho-oto/mascii-vscode](https://github.com/ho-oto/mascii-vscode) | webview パネル |
| Obsidian | [ho-oto/mascii-obsidian](https://github.com/ho-oto/mascii-obsidian) | モーダル + pkg-node |
| Zed | (下記の CLI タスク) | 拡張 UI API 待ち |

```
     mascii (このリポジトリ: ast/render/parse/editor + CLI)
              │                          │
              │ git 依存                 │ git 依存
              ▼                          ▼
     mascii-vscode/wasm         mascii-obsidian/wasm
              │                          │
       webview パネル                モーダル
```

各拡張リポジトリは**自前の wasm-bindgen クレート**を持ち、`mascii` を
git 依存で引く。共有クレートを跨がないので、`npm run build` だけで
その場でビルド・動作確認できる。バインディング(`MasciiEditor` +
変換 API)は各リポジトリに複製されている — 意図的な重複で、
リポジトリ独立性と引き換えにしている。

- エディタ本体のロジック(カーソル・コマンド・選択)はすべて Rust 側
  (`src/editor/mod.rs`)にあり、JS はキーイベント転送と画面表示だけを行う。
  `MasciiEditor.screen()` はカーソル `▌`・選択(各セルに結合下線 U+0332)込みのテキストを返す。
- フェンス検出は各エディタ側で行う(````math` / ````mascii`)。
  確定時にブロック内容を正準 AA で書き戻す。
- wasm クレートは `mascii` を `default-features = false` で参照するので
  ratatui(TUI 専用、feature "tui")は wasm ビルドに含まれない。

## Zed

Zed の拡張機構(2025 時点)はテーマ・言語・言語サーバー中心で、
VSCode の webview に相当するカスタム UI パネルをまだ提供していない。
そのため CLI 経由で使う:

```json
// .zed/tasks.json  (cargo install --path . で mascii を PATH に入れておく)
[
  { "label": "mascii: AA → LaTeX", "command": "printf '%s' \"$ZED_SELECTED_TEXT\" | mascii aa2tex", "reveal": "always" },
  { "label": "mascii: 整形(正準AA)", "command": "printf '%s' \"$ZED_SELECTED_TEXT\" | mascii fmt", "reveal": "always" }
]
```

内蔵ターミナルで `mascii` を起動して編集し、Ctrl+Y で AA をコピーする
経路もある。拡張 UI API が公開されたら、VSCode 版と同じ構成
(自前 wasm クレート + git 依存)でそのまま実装できる。

## 動作確認の状況

- `mascii-wasm` は wasm32 でビルドでき、Node 上で
  変換 API と `MasciiEditor`(キー駆動で x²+1 を構築 → screen/aa/latex)
  の動作を確認済み。
- VSCode / Obsidian 拡張は**プロトタイプ**(コードレビュー済み・実機未検証)。
  受け入れテスト: 拡張を dev モードで起動 → ```math ブロックで
  Ctrl+Alt+M → 編集 → Ctrl+Enter で書き戻し。

## ロードマップ(インライン体験へ)

「数式部分だけがその場で構造エディタになる」理想形への段階:

1. **今**: フェンスブロック + 別パネル/モーダルで編集(実装済み)。
2. Obsidian: CodeMirror6 の `ReplaceDecoration` ウィジェットで、
   Live Preview 中の ```math ブロックを `MasciiEditor` の DOM に差し替える
   (読み書きとも CM6 トランザクション経由)。技術的に最短。
3. VSCode: フェンス部分の装飾(`setDecorations`)+ カーソル進入時に
   インラインウェビュー…は API がないため、`TextEditorEdit` を
   キーストロークごとに適用する「仮想構造編集モード」か、
   Notebook Renderer / Custom Editor で md 全体を扱う方式を検討。
4. Zed: 拡張 UI API(パネル/スラッシュコマンド)公開待ち。
