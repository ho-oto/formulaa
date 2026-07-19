# mascii

LyX 風の操作感で数式をインタラクティブに編集する TUI エディタ、および
アスキーアート数式(Unicode 数式記号優先)と数式 AST の**双方向変換器**。

数式は AST と一対一対応する「正準AA形式」として描画され、そのまま
パースして LaTeX / Typst に機械変換できます。プレーンテキストの世界で
数式を考え、必要になったら組版系に持っていく、を成立させるツールです。

```
        ______
    −𝑏±√𝑏²−4𝑎𝑐          ∞   1    π²        ⎡ cosθ   −sinθ ⎤
 𝑥=────────────       ┄┄∑┄┄────=────   𝑅 = ⎢      ┼       ⎥
        2𝑎             𝑛=1  𝑛²   6         ⎣ sinθ   cosθ  ⎦
```

## 使い方

```sh
cargo run                       # TUI エディタ(Ctrl+S で formula.tex に保存)
cargo run -- out.tex            # 保存先を指定

# 変換 CLI(ファイルまたは標準入力)
mascii aa2tex   formula.txt     # AA → LaTeX
mascii aa2typst formula.txt     # AA → Typst
mascii fmt      formula.txt     # AA → 正準AA(手書きAAの正規化・整形)

cargo run --example demo        # レンダリングのサンプルを表示
```

手書きAAもある程度受理します(ASCII の `x+1`、`E=mc²`、`┄` の代わりの
`~∑~` バンドなど)。

TUI は毎編集後に AA→AST の逆変換を自動検査し、ラウンドトリップが壊れる
編集を見つけると `mascii_debug/roundtrip-N.txt` にレポートを保存します
(ステータスバーに ⚠ 表示)。バグ報告にそのまま使えます。

## キー操作(LyX 風)

| キー | 動作 |
|---|---|
| 英数字・演算子 | そのまま挿入(表示は数学イタリック体・Unicode 記号) |
| `\` | コマンドミニバッファ(`\frac`␣ `\sum`␣ `\alpha`␣ `\bbR`␣ など) |
| `^` / `_` | 上付き / 下付き(可能なら `x²` `aᵢ` とインライン表示) |
| `(` / `)`、`{` / `}` | 自動サイズ括弧に入る / 抜ける |
| `]` | 行列から抜ける(`[` `]` 自体は行列予約) |
| `Tab` | 現在のインセット(分数・根号・極限など)から抜ける |
| `Space` | 整形用の空白(AAにだけ現れ、LaTeX/Typst には出ない。意味のある空白は `\space`→␣) |
| `←` `→` | 構造の中を通り抜けながら移動 |
| `↑` `↓` | 分子⇄分母、極限の上⇄下、行列の行間移動 |
| `Enter` | 行列/グリッド内: 下に行を追加(`\addcol` `\delrow` `\delcol` も) |
| `Backspace` | 中身のある構造には踏み込み、空の構造は削除 |
| `Shift+←` `→` | ブロック選択(兄弟ノード範囲、⟦ ⟧ 表示)。選択中に `\cancel` `\frac` `\sqrt` `^` `_` `(` で包む、`Backspace` でまとめて削除 |
| `Ctrl+G` | ジャンプモード(EasyMotion 風): 全編集位置にラベルを表示し、ラベルキーで移動 |
| `Ctrl+B` | ブロック強調: カーソルを含む構造とその親・祖父を色付き ⟨ ⟩ で表示 |
| `Ctrl+O` | 構造ビュー: 全ブロックの入れ子構造を深さ別の背景色で一気に表示 |
| `Ctrl+Y` | 正準AAをクリップボードへコピー |
| `Ctrl+S` | LaTeX を保存 / `Ctrl+T` イタリック表示切替 / `Ctrl+Q` 終了 |

## コマンド(抜粋)

- 構造: `\frac` `\sqrt` `\cbrt`(∛) `\matrix`(2×2。`\matrix34` で 3行×4列、
  各 *matrix/cases/array 共通。Enter/\addcol などで後から増減も可)
- デリミタ: `\pmatrix` `\Bmatrix` `\vmatrix` `\array`(裸グリッド)`\cases`
  `\abs` `\langle` `\braket`(⟨·|·⟩)`\set`({·|·})`\mid`(セグメント分割)
  `\delim<left><right>[mids]`(例 `\delim(]`、`< >` は ⟨ ⟩ の別名)
- 大型演算子: `\sum` `\prod` `\int` `\oint` `\bigcup` …(挿入直後は下極限、
  `↑` で上極限へ)。`\lim` `\max` `\inf` `\det` `\Pr` なども同じバンドで
  下極限に入る(`┄lim┄`)
- 関数名: `\sin` `\cos` `\log` `\lim` …(立体で表示)
- アクセント: `\hat` `\vec` `\bar` `\dot` `\tilde` `\underline`
  (直前の1文字に付く。続けて実行すると縦に重ね掛け)
- 伸縮矢印: `\xto` `\xfrom`(→ ←)、`\xTo` `\xFrom`(⇒ ⇐)— 上下にラベル
- テキスト: `\rmdx` → `"dx"`(\mathrm)、`\text...`(空白入りは AA 上 ␣)
- 記号: 厳選テーブル + [ho-oto/mathematical-symbols](https://github.com/ho-oto/mathematical-symbols)
  由来の 4000+ エントリ(`\bbR`→ℝ, `\->`→→, `\oo`→∞ など)

## フォントについて

Unicode 数式記号は多くの等幅フォントで欠けていたり等幅でなかったりします。
対策は**合成フォント**: [JuliaMono](https://juliamono.netlify.app/) は数式記号の
カバレッジが非常に広い等幅フォント。`tools/merge_math_font.py` で、任意の
等幅フォントの不足グリフを JuliaMono から補い、送り幅をベースフォントの
セル幅に正確に揃えた合成フォントを生成できます:

   ```sh
   pip install fonttools
   python3 tools/merge_math_font.py /System/Library/Fonts/Menlo.ttc \
       -j JuliaMono-Regular.ttf -o Menlo-Math.ttf
   ```

## エディタ統合(VSCode / Obsidian / Zed)

Rust コアを WASM 化(`wasm/`)し、Markdown の ```math フェンス内の
AA 数式を構造エディタで編集する拡張のプロトタイプを `editors/` に用意:

- **VSCode**: `Ctrl+Alt+M` でカーソル位置の数式を webview エディタで開き、
  `Ctrl+Enter` で書き戻し。選択 AA の LaTeX/Typst 変換コマンドも。
- **Obsidian**: コマンド「Edit mascii formula at cursor」でモーダル編集。
- **Zed**: 拡張 UI API 未提供のため CLI タスク連携(選択→aa2tex 等)。

ビルド手順と「数式部分だけその場で構造編集」への段階的ロードマップは
`docs/editors.md` 参照。

## 設計

内部表現は TeX 文字列ではなく**数式 AST**。構造編集・AA 描画・LaTeX/Typst
出力のすべてが AST から導出されます。AA は AST と一対一対応する正準形式で、
ラウンドトリップ性(`parse ∘ render == id`)を実式コーパスとランダム生成
2000 ケースのプロパティテストで保証しています。

- `docs/aa-spec.md` — 正準AA形式の仕様(予約グリフ・レイアウト・パース規則)
- `docs/design.md` — 設計判断の経緯とロードマップ
- `CLAUDE.md` — 開発を引き継ぐ AI/人間向けのガイド

```
src/ast.rs      AST・カーソルパス・正規形
src/render.rs   AST → 2D 文字ブロック(正準AA)
src/parse.rs    AA → AST(逆変換)
src/editor.rs   構造エディタ(LyX 型)
src/latex.rs    AST → LaTeX      src/typst.rs  AST → Typst
src/symbols.rs  記号・関数・アクセント表(symbols_ext.rs は生成物)
src/main.rs     ratatui TUI + CLI
```

## ロードマップ

docs/design.md 参照。直近: 縦棒デリミタ(|x|・行列式)、可変幅アクセント
(`\overline{x+y}`)、`\lim` の下極限、LaTeX パーサ(逆方向)、MathML 出力。
