# mascii — AI 開発者向けガイド

LyX 風 TUI 数式エディタ + AA⇄AST 相互変換ツール。Rust / ratatui。

## コマンド

```sh
cargo test                      # 全テスト(ユニット + ラウンドトリップ)
cargo check -p mascii-wasm --target wasm32-unknown-unknown  # wasm 側
cargo clippy --all-targets     # 警告ゼロを維持
cargo fmt                      # コミット前に必ず整形(リポジトリ全体が rustfmt 準拠)
cargo run                      # TUI エディタ
cargo run --example demo       # レンダリングのサンプル出力(TUI なし)
cargo run --example ambig      # バンド記法が解決した曖昧性の回帰デモ
echo '...' | cargo run -q -- aa2tex    # AA → LaTeX(aa2typst / fmt も同様)
```

## 最重要ルール

1. **`render.rs` と `parse.rs` は仕様(docs/parse-model.md / aa-spec.md)の表と裏。**
   片方だけ変更してはならない。変更したら `cargo test` で
   ラウンドトリップ契約を確認:
   `parse(render(normalize(x))) == normalize(strip_spacers(normalize(x)))`
   (`render(parse(aa)) == aa` は要求ではない — AA はソースコードで、
   受理は正準形より広い。fmt が整える)
2. 新しい描画グリフを導入するときは docs/aa-spec.md の予約グリフ表を更新し、
   `symbols/mod.rs`(is_reserved_glyph)/ `symbols/ext.rs` に原子として同じ文字が存在しないことを確認。
3. `normalize`(ast.rs)は**冪等**でなければならない(合流後の再正規化)。
4. 機能追加はまず `tests/roundtrip.rs` に実式を足してから実装する。
   キー操作の変更は `tests/ui.rs`(キースクリプト)に足す。キーの意味は
   `src/input.rs` の共有キーマップだけが決める — main.rs / wasm に
   キー分岐を書き足してはならない(ドリフトの元)。
   ランダムプロパティテスト(2000件、`MASCII_PROP_N`/`MASCII_PROP_SEED` で
   増量可)が回帰を検出してくれる。
5. TUI は毎編集後にラウンドトリップを自動検査し、失敗すると
   **`mascii_debug/roundtrip-N.txt`**(gitignore 済み)にレポートを吐く。
   ユーザーから「バグった」と言われたらまずこのディレクトリを読むこと。
   レポートには正準AA・期待AST・パース結果AST・両者の LaTeX が入っている。
   修正したら同じ AA を `tests/roundtrip.rs` に回帰として追加する。

## モジュール地図

| ファイル | 役割 |
|---|---|
| `src/ast.rs` | 数式 AST(`Node`/`Row`/`Field`)、カーソルパス、`normalize` |
| `src/render.rs` | AST → 2D ブロック(基線つき)。正準AAの生成側 |
| `src/parse.rs` | AA → AST。領域+基線の再帰下降。正準AAの受理側+寛容入力 |
| `src/editor.rs` | 構造エディタ(LyX 型カーソル、コマンド実行) |
| `src/input.rs` | **共有キーマップ**(`Key`/`Effect`/`Editor::input`)。TUI と wasm は変換だけ |
| `src/output/latex.rs` / `src/output/typst.rs` | AST → LaTeX / Typst(crate ルートの `mascii::latex`/`typst` で再輸出) |
| `src/symbols/mod.rs` | 厳選シンボル表・関数表 FUNCS(limits/LaTeX/Typst フラグ付き)・BIG_OPS・アクセント表・予約グリフ判定 |
| `src/symbols/ext.rs` | **生成物**(ho-oto/mathematical-symbols 由来、696件)。phf マップ(順序不要・重複はビルドエラー)。手編集しない |
| `src/symbols/alphabets.rs` | スタイル付きアルファベット族(`\bbR` `\Afrk` `\bfsf3` …12族28綴り×前置/後置)を規則+例外表で表現 |
| `src/theme.rs` | TUI の配色定数(bin 専用) |
| `src/main.rs` | TUI(ratatui)+ CLI サブコマンド |
| `tests/roundtrip.rs` | 実式コーパス + ランダムプロパティテスト |
| `tests/ui.rs` | キー駆動 UI テスト(キースクリプト DSL + ランダムキー列。`MASCII_UI_PROP_N`/`_SEED`) |
| `tools/merge_math_font.py` | JuliaMono から不足数式グリフを補う合成フォント生成(fontTools) |
| `wasm/` | wasm-bindgen バインディング(変換 API + キー駆動 `MasciiEditor`) |
| `editors/` | VSCode / Obsidian / Zed 統合(docs/editors.md 参照) |
| `SKILL.md` | AI が AA を直接読み書きするためのガイド |
| `docs/examples.md` | コーパス対照表(examples/catalog.rs で再生成) |

## 設計文書

- `docs/aa-spec.md` — 正準AA形式の仕様(グリフ・ノード別レイアウト規則)
- `docs/parse-model.md` — パースモデル仕様(基線復元→走査→再帰の視点。契約と fuse 表)
- `docs/design.md` — 設計判断の経緯とロードマップ。**着手前に必読**
- `docs/jump-spec.md` — ^G ジャンプ v2 の仕様草案(候補選抜アルゴリズム)
- `docs/keys.md` — ユーザー向けキーマニュアル。**キー操作を変えたら必ず更新**

## ハマりどころ

- `Block.baseline` は上付きブロックでは `height()` と等しくなる(基線行が
  存在しない)。`lines[baseline]` を無条件に索引しない。
- 行内の「全高空白列」は同一基線の兄弟の区切りとしてのみ許される。
  構造ブロックに無条件マージンを足すとスクリプト分割が壊れる
  (docs/design.md §9)。
- カーソルは `Block.caret`(幅ゼロのメタデータ、全合成で伝搬)。描画は
  カーソル有無でジオメトリが変わらない(TUI は反転表示、wasm のテキスト
  画面だけ ▌ を上書き描画)。パース対象はカーソルなし描画のみ。
  render_node の新アームでは caret の伝搬を忘れない(cancel と同じ
  オフセットで写す。忘れると tests/ui.rs の caret 生存チェックが落ちる)。
- `Sqrt` は `index: u8`(2/3/4 = √∛∜)を持つ。`Node::Accent` の base は
  1 文字(Row ではない)。
- 括弧は `Node::Delim{left,right,mids,segs}` に統一(旧 Paren/Matrix は廃止)。
  `Node::Array` はどこでも格子(裸なら ┌┬┐ フルフレーム、デリミタの単独 seg
  なら最小マーカーの融合形 — ┼ 区切り行 / ┬┴ 行 / ├┤ 接合。
  波括弧は融合しない)。空白の個数に
  依存する規則は存在しない(docs/parse-model.md §0)。
- Space は整形スペーサ(再パースで消える)、`\space`=␣、脱出は Tab、
  Enter はグリッド内で行追加・トップレベルで数式改行(`Node::Break`、
  行間に `┈` 単体の区切り行)、Ctrl+Y で AA をクリップボードへ。
  トップレベルの描画入口は `render_root`(Break 分割+縦積み)——
  ルート行を描くときに `render_row` を直接呼ばない。
- ratatui は feature "tui"(bin 専用)。ライブラリ本体に TUI 依存を
  持ち込まない(wasm ビルドが壊れる)。
- ratatui のイベントは `KeyEventKind::Press` のみ処理(Windows の重複対策)。
- ジャンプ(Ctrl+G)・ブロック選択(Ctrl+B)・選択範囲は私用領域文字の
  マーカー原子を表示用クローン AST に挿入する方式(editor.rs `decorated`)。
  U+E000–E0FF は表示マーカー予約。マーカー原子は**幅ゼロで描画**され
  `Block.marks` として伝搬(レイアウト不変)。TUI 側は marks から
  ラベル重ね書きと背景ボックスを塗る(main.rs `marker_boxes`)。
  構造ビュー(Ctrl+O)は正準描画を `parse_with_regions` に通して
  矩形+深さを回収し背景色を塗る(main.rs `draw_structure`)。
