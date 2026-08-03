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
echo '...' | cargo run -q -- aa2tex    # AA → LaTeX(fmt も同様)
```

## 最重要ルール

1. **`render.rs` と `parse.rs` は仕様(docs/parse-model.md / aa-spec.md)の表と裏。**
   片方だけ変更してはならない。変更したら `cargo test` で
   ラウンドトリップ契約を確認:
   `parse(render(normalize(x))) == normalize(strip_spacers(normalize(x)))`
   (`render(parse(aa)) == aa` は要求ではない — AA はソースコードで、
   受理は正準形より広い。fmt が整える)
2. 新しい描画グリフを導入するときは docs/aa-spec.md の予約グリフ表を更新し、
   `symbols/atoms.rs` のテスト `reserved_glyphs_stay_out_of_the_tables` の
   予約表(`is_reserved_glyph`)に追加する(原子テーブルとの衝突はテストが検出)。
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
| `src/render/` | AST → 2D ブロック(基線つき)。正準AAの生成側。`block.rs` は Node 非依存のブロック代数(4チャンネル伝搬のユニットテスト付き)、`mod.rs` がノード規則 |
| `src/parse.rs` | AA → AST。領域+基線の再帰下降。正準AAの受理側+寛容入力 |
| `src/editor/mod.rs` | 構造エディタ(LyX 型カーソル、挿入・移動・削除・選択) |
| `src/editor/modes.rs` | ^F フリーカーソル・^G ジャンプ・^B ブロック選択・^O グリッド編集(セル矩形選択+行/列レーンモード `GridSel`)・表示装飾 |
| `src/editor/command.rs` | **`Edit` enum + `resolve`/`apply`**(綴り→編集の純粋解決と適用の分離)、`\op` 名前ボックス |
| `src/input.rs` | **共有キーマップ**(`Key`/`Effect`/`Editor::input`)。TUI と wasm は変換だけ。木を変えるキーは `Edit` を組んで `apply` に流す(モード・ナビゲーション・文脈キー `// ) ] }` はキー層) |
| `src/output/latex.rs` | AST → LaTeX(crate ルートの `mascii::latex` で再輸出)。スクリプトを吸収しうるノードは `{…}` で保護(往復のため) |
| `src/from_latex.rs` | **LaTeX → AST**(第2経路)。自前出力は完全往復(roundtrip ハーネスが検証)、外部 LaTeX(KaTeX/MathJax 方言)は best-effort で不明要素をスキップ。`\tex` ボックス・`tex2aa`・wasm `latex_to_aa` が使う |
| `src/symbols/` | **全テーブルの家**(1関心1ファイル、`symbols::X` でフラットに再輸出) |
| ├ `atoms.rs` | **全記号語彙のテーブル3枚**(すべて phf・手書き): 出力側 `ATOMS`(char → LaTeX 綴り+`kind` Sym/BigOp)、入力側 `NAMES`(綴り → char、別綴り・ASCII 絵文字綴りは `\|` で併記。コマンド別綴りは `resolve` の match パターン)、否定 `NEGATIONS`(基底 char → 斜線付き char。`!` 前置/後置綴りの解決元、網羅性はテーブル走査のテストが固定)・予約グリフ・`is_atom`。**入力できる原子は必ず LaTeX 綴りを持つ**(gap=0 をテストが固定) |
| ├ `funcs.rs` | 立体関数 `FUNCS`(limits/spaced)。∑系は `ATOMS` の kind に統合 |
| ├ `accents.rs` | `Accent` enum: 入力 `ACCENT_NAMES`(phf, 綴り→variant)+`info()`(variant→全属性の1 match) |
| ├ `radicals.rs` | `Radical` enum: 入力 `RADICAL_NAMES`(phf)+`info()`(グリフ・LaTeX 指数) |
| ├ `delims.rs` | `Delim { Col(ColDelim), Angle }`: 柱型7種は `ColDelim`(`info()` に仕様文字・1行/縦グリフ・LaTeX、`tall` は非Option)、対角腕の Angle は型で別扱い(bra-ket `⟨x\|` のため別ノードにはしない)。入力 `DELIM_SPECS`(phf)/`DELIM_NAMES`+列分類(`of_run`/`run_glyphs`/`fuses` — `ColDelim` 側)。parse/render/latex/tui がここを引く |
| ├ `arrows.rs` | `Arrow` enum: 入力 `ARROW_NAMES`(phf, 綴り→variant)+`info()`(variant→全属性の1 match) |
| ├ `grids.rs` | 行列環境の対応表 `GRID_ENVS`(コマンド名/env 名 → `GridWrap`)。editor と LaTeX 両方向が読む |
| ├ `scripts.rs` | インライン上付き/下付き: phf 3枚(base→sup・base→sub・script→base)、テストが全単射を固定 |
| ├ `alphabets.rs` | スタイル族(12族28綴り×前置/後置)を phf(別綴りは or キー)+規則+例外表で。LaTeX 逆引き(`𝔸`→`\mathbb{A}`)もここ |

| `src/glyphs.rs` | **構造グリフ定数+表示マーカー**(`Mark` enum: `ch`/`decode`/`opener` が私用領域の唯一の綴り。TUI・wasm はこの enum を match する)。以下は定数: 格子(3×3 junction 表・辺・融合マーカー名)・基線マーク `─ ┈ ═ ⬚`・norm/mid/角括弧腕・根号の茎/庇・brace 角 ╭╮╰╯・カーソル文字。symbols は「phf テーブル+分類 enum」専用 |
| `src/theme.rs` | TUI の配色定数(bin 専用) |
| `src/main.rs` | メインループ + CLI サブコマンド |
| `src/tui.rs` | 描画(レイアウト・スクロール・マーカー/選択の塗り・セル装飾) |
| `src/guard.rs` | 編集ごとのラウンドトリップ自動検査とレポート出力 |
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
  render_node の新アームでは caret の伝搬を忘れない(忘れると
  tests/ui.rs の caret 生存チェックが落ちる)。
- `Sqrt` の `index`・矢印の `op`・アクセントの `overs`/`unders`・括弧の
  `left`/`right` は enum(`Radical`/`Arrow`/`Accent`/`Delim`)。`Node::Accent` の base は
  1 文字(Row ではない)。打ち消し線(\cancel)は非対応 — 否定は
  合成済み斜線付き原子(≠ ∉ …。`!` 前置/後置の綴りは `symbols::negated`
  経由で解決)。
- 括弧は `Node::Delim{left,right,mids,segs}` に統一(旧 Paren/Matrix は廃止)。
  `left`/`right` は `Delim` enum(スロットが側を決める — `]` は左に置けない)、
  `mids` は │ の本数。
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
  マーカー原子を表示用クローン AST に挿入する方式(editor/modes.rs `decorated`)。
  私用領域 **U+E000–F8FF 全域**が表示用予約(`glyphs::is_display_marker`):
  E000+ ジャンプ/^B ラベル、E0F0–E0FF 選択・グリッドのセル/レーン対・
  フレーム角・隙間ゴースト、E100+ ランク、F000+ 座標プローブ、
  F8F0/F8F1 は tui のボックス囲み。**生のまま端末に出してはならない**
  (フォントが私用領域にロゴを持つ — decorate_line が描画前に必ず
  グリフへ解決する)。
  マーカー原子は**幅ゼロで描画**され
  `Block.marks` として伝搬(レイアウト不変)。TUI 側は marks から
  ラベル重ね書きと背景ボックスを塗る(tui.rs `marker_boxes`)。
  **例外**: グリッド編集の隙間カーソルだけは装飾 AST に幅1の
  ゴーストレーン(Spacer セル)を実体挿入する — 挿入プレビューを
  兼ねるための意図的なレイアウト変化で、クリック座標のプローブ描画も
  同じゴーストを含めて整合させる(editor/mod.rs `display_coords`)。
