# エディタ統合 (VSCode / Obsidian / Zed)

目標体験: 「基本はテキストとして編集し、数式になっている部分だけは
TUI エディタ相当の構造編集ができる」。数式は Markdown の
` ```math ` フェンスブロックに AA 形式で埋め込む(AA は正準形式なので、
いつでも再パースして構造編集・LaTeX 変換ができる)。

## アーキテクチャ

```
            mascii (Rust ライブラリ: ast/render/parse/editor)
                     │  wasm-pack
                     ▼
        wasm/  mascii-wasm (wasm-bindgen バインディング)
          ├── aa_to_latex / latex_to_aa / aa_format / aa_check
          └── MasciiEditor: key(key, shift) で駆動する構造エディタ
                     │
   ┌─────────────────┼──────────────────────┐
   ▼                 ▼                      ▼
editors/vscode   editors/obsidian       editors/zed
(webview パネル)  (モーダル + pkg-node)   (CLI タスク; UI API 待ち)
```

- エディタ本体のロジック(カーソル・コマンド・選択)はすべて Rust 側
  (`src/editor/mod.rs`)にあり、JS はキーイベント転送と画面表示だけを行う。
  `MasciiEditor.screen()` はカーソル `▌`・選択(各セルに結合下線 U+0332)込みのテキストを返す。
- フェンス検出は各エディタ側で行う(````math` / ````mascii`)。
  確定時にブロック内容を正準 AA で書き戻す。

## ビルド

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack            # 未導入なら

# VSCode 用 (web target)
cd wasm && wasm-pack build --target web --out-dir ../editors/vscode/media/pkg

# Obsidian / Node 用 (nodejs target)
cd wasm && wasm-pack build --target nodejs --out-dir ../editors/obsidian/pkg-node
```

各ディレクトリの README にインストール手順がある。
wasm クレートは `mascii` を `default-features = false` で参照するので
ratatui(TUI 専用、feature "tui")は wasm ビルドに含まれない。

## 動作確認の状況

- `mascii-wasm` は wasm32 でビルドでき、Node 上で
  変換 API と `MasciiEditor`(キー駆動で x²+1/2 を構築 → screen/aa/latex)
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
   wasm コアはそのまま流用可能。
