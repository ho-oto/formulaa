# mascii — AI 開発者向けガイド

LyX 風 TUI 数式エディタ + AA⇄AST 相互変換ツール。Rust / ratatui。

## コマンド

```sh
cargo test                      # 全テスト(ユニット + ラウンドトリップ)
cargo check -p mascii-wasm --target wasm32-unknown-unknown  # wasm 側
cargo clippy --all-targets     # 警告ゼロを維持
cargo run                      # TUI エディタ
cargo run --example demo       # レンダリングのサンプル出力(TUI なし)
cargo run --example ambig      # バンド記法が解決した曖昧性の回帰デモ
echo '...' | cargo run -q -- aa2tex    # AA → LaTeX(aa2typst / fmt も同様)
```

## 最重要ルール

1. **`render.rs` と `parse.rs` は正準AA仕様(docs/aa-spec.md)の表と裏。**
   片方だけ変更してはならない。変更したら `cargo test` で
   ラウンドトリップ不変条件を確認:
   `parse(render(normalize(x))) == normalize(x)` / `render(parse(aa)) == aa`
2. 新しい描画グリフを導入するときは docs/aa-spec.md の予約グリフ表を更新し、
   `symbols.rs` / `symbols_ext.rs` に原子として同じ文字が存在しないことを確認。
3. `normalize`(ast.rs)は**冪等**でなければならない(合流後の再正規化)。
4. 機能追加はまず `tests/roundtrip.rs` に実式を足してから実装する。
   ランダムプロパティテスト(2000件)が回帰を検出してくれる。

## モジュール地図

| ファイル | 役割 |
|---|---|
| `src/ast.rs` | 数式 AST(`Node`/`Row`/`Field`)、カーソルパス、`normalize` |
| `src/render.rs` | AST → 2D ブロック(基線つき)。正準AAの生成側 |
| `src/parse.rs` | AA → AST。領域+基線の再帰下降。正準AAの受理側+寛容入力 |
| `src/editor.rs` | 構造エディタ(LyX 型カーソル、コマンド実行) |
| `src/latex.rs` / `src/typst.rs` | AST → LaTeX / Typst |
| `src/symbols.rs` | 厳選シンボル表・関数名辞書・アクセント表・LaTeX 逆引き |
| `src/symbols_ext.rs` | **生成物**(ho-oto/mathematical-symbols 由来、4000+)。手編集しない |
| `src/main.rs` | TUI(ratatui)+ CLI サブコマンド |
| `tests/roundtrip.rs` | 実式コーパス + ランダムプロパティテスト |
| `tools/merge_math_font.py` | JuliaMono から不足数式グリフを補う合成フォント生成(fontTools) |
| `wasm/` | wasm-bindgen バインディング(変換 API + キー駆動 `MasciiEditor`) |
| `editors/` | VSCode / Obsidian / Zed 統合(docs/editors.md 参照) |
| `SKILL.md` | AI が AA を直接読み書きするためのガイド |
| `docs/examples.md` | コーパス対照表(examples/catalog.rs で再生成) |

## 設計文書

- `docs/aa-spec.md` — 正準AA形式の仕様(グリフ・レイアウト規則・パース規則)
- `docs/design.md` — 設計判断の経緯とロードマップ。**着手前に必読**

## ハマりどころ

- `Block.baseline` は上付きブロックでは `height()` と等しくなる(基線行が
  存在しない)。`lines[baseline]` を無条件に索引しない。
- 行内の「全高空白列」は同一基線の兄弟の区切りとしてのみ許される。
  構造ブロックに無条件マージンを足すとスクリプト分割が壊れる
  (docs/design.md §9)。
- エディタのカーソル表示(`▌`)がある描画は正準形ではない。パース対象は
  カーソルなし描画のみ。
- `Sqrt` は `index: u8`(2/3/4 = √∛∜)を持つ。`Node::Accent` の base は
  1 文字(Row ではない)。
- ratatui は feature "tui"(bin 専用)。ライブラリ本体に TUI 依存を
  持ち込まない(wasm ビルドが壊れる)。
- ratatui のイベントは `KeyEventKind::Press` のみ処理(Windows の重複対策)。
- ジャンプ(Ctrl+G)・ブロック強調(Ctrl+B)は私用領域文字のマーカー原子を
  表示用クローン AST に挿入する方式(editor.rs `decorated`)。U+E000–E0FF は
  表示マーカー予約。構造ビュー(Ctrl+O)は正準描画を `parse_with_regions` に
  通して矩形+深さを回収し背景色を塗る(main.rs `draw_structure`)。
