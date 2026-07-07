# mascii

LyX 風の操作感で数式をインタラクティブに編集する TUI エディタ、および
アスキーアート数式(Unicode 数式記号優先)と数式 AST の**双方向変換器**。

数式は AST と一対一対応する「正準AA形式」として描画され、そのまま
パースして LaTeX / Typst に機械変換できます。プレーンテキストの世界で
数式を考え、必要になったら組版系に持っていく、を成立させるツールです。

```
               ________
        − 𝑏 ± √𝑏² − 4𝑎𝑐            ∞   1      π²        ⎡cos𝜃  −sin𝜃⎤
  𝑥 = ──────────────────         ┄┄∑┄┄──── = ────    R = ⎢          ⎥
              2𝑎                  𝑛=1  𝑛²     6          ⎣sin𝜃  cos𝜃 ⎦
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

手書きAAもある程度受理します(ASCII の `x+1`、`E=mc²` など)。基線が
曖昧な場合は行頭に `▶` を置いて基線行を明示できます。

## キー操作(LyX 風)

| キー | 動作 |
|---|---|
| 英数字・演算子 | そのまま挿入(表示は数学イタリック体・Unicode 記号) |
| `\` | コマンドミニバッファ(`\frac`␣ `\sum`␣ `\alpha`␣ `\bbR`␣ など) |
| `^` / `_` | 上付き / 下付き(可能なら `x²` `aᵢ` とインライン表示) |
| `(` / `)` | 自動サイズ括弧に入る / 抜ける |
| `]` | 行列から抜ける(`[` `]` 自体は行列予約) |
| `Space` | 現在のインセット(分数・根号・極限など)から抜ける |
| `←` `→` | 構造の中を通り抜けながら移動 |
| `↑` `↓` | 分子⇄分母、極限の上⇄下、行列の行間移動 |
| `Backspace` | 中身のある構造には踏み込み、空の構造は削除 |
| `Ctrl+S` | LaTeX を保存 / `F2` イタリック表示切替 / `Ctrl+Q` 終了 |

## コマンド(抜粋)

- 構造: `\frac` `\sqrt` `\cbrt`(∛) `\matrix`(2×2)
- 大型演算子: `\sum` `\prod` `\int` `\oint` `\bigcup` …(挿入直後は下極限、
  `↑` で上極限へ)
- 関数名: `\sin` `\cos` `\log` `\lim` …(立体で表示)
- アクセント: `\hat` `\vec` `\bar` `\dot` `\tilde` `\underline`
  (直前の1文字に付く)
- 記号: 厳選テーブル + [ho-oto/mathematical-symbols](https://github.com/ho-oto/mathematical-symbols)
  由来の 4000+ エントリ(`\bbR`→ℝ, `\->`→→, `\oo`→∞ など)

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
