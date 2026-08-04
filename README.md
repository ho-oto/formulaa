# mascii

LyX 風の操作感で数式をインタラクティブに編集する TUI エディタ、および
アスキーアート数式(Unicode 数式記号優先)と数式 AST の**双方向変換器**。

数式は AST と一対一対応する「正準AA形式」として描画され、そのまま
パースして LaTeX に機械変換できます。プレーンテキストの世界で
数式を考え、必要になったら組版系に持っていく、を成立させるツールです。

```
          ┌────────
     -𝑏 ± √𝑏² - 4𝑎𝑐        ∞     1     π²         ⎡ cos θ   -sin θ ⎤
𝑥 = ────────────────     ┈┈∑┈┈ ──── = ────    𝑅 = ⎢        ┼       ⎥
            2𝑎            𝑛=1    𝑛²     6         ⎣ sin θ   cos θ  ⎦
```

## 使い方

```sh
cargo run                       # TUI エディタ(^Y で AA をクリップボードへ)

# 変換 CLI(ファイルまたは標準入力)
mascii aa2tex   formula.txt     # AA → LaTeX
mascii fmt      formula.txt     # AA → 正準AA(手書きAAの正規化・整形)

cargo run --example demo        # レンダリングのサンプルを表示

```

手書きAAもある程度受理します(ASCII の `x+1`、`E=m c²` など。英字は
単独1文字=イタリック変数、2文字以上のランは辞書語なら関数・それ以外は
\operatorname、単独1文字は \mathrm)。

TUI は毎編集後に AA→AST の逆変換を自動検査し、ラウンドトリップが壊れる
編集を見つけると `mascii_debug/roundtrip-N.txt` にレポートを保存します
(ステータスバーに ⚠ 表示)。バグ報告にそのまま使えます。

## キー操作

完全なリファレンスは **[docs/keys.md](docs/keys.md)**(全キー・全コマンド・全モード)。(LyX 風)

| キー | 動作 |
|---|---|
| 英数字・演算子 | そのまま挿入(表示は数学イタリック体・Unicode 記号) |
| `\` | コマンドミニバッファ(`\frac`␣ `\sum`␣ `\alpha`␣ `\bbR`␣ など) |
| `^` / `_` | 上付き / 下付き(可能なら `x²` `aᵢ` とインライン表示) |
| `(` `[` `{` / `)` `]` `}` | 自動サイズ括弧に入る / 抜ける(`]` は行列からの脱出も) |
| `//` | 分数(`/` 2連打。リテラル `//` は `/ /` と打って間の空白を消す) |
| `Tab` | 現在のインセット(分数・根号・極限など)から抜ける |
| `←` `→` | 構造の中を通り抜けながら移動(選択中は選択の左端/右端に集約) |
| `Ctrl+A` | 数式全体の先頭へ(`Home`/`End` は現在行の先頭/末尾) |
| `Ctrl+Z` / `Ctrl+R` | undo / redo(数式が変わった操作ごとに1ステップ。カーソルも一緒に戻る) |
| `Ctrl+O`(行列内) | グリッド編集モード(矢印=セル移動、r/R c/C 追加、d/D 削除) |
| マウスクリック | クリック位置に最も近い編集位置へカーソル移動(端末のテキスト選択は Shift+ドラッグに変わります) |
| `↑` `↓` | 分子⇄分母、極限の上⇄下、行列の行間移動。直前が裸の大型演算子(∑ や lim)ならバンドに昇格して極限に入る。複数行数式では行間移動(列位置を保持) |
| `Enter` | 行列/グリッド内: 下に行を追加(`\addcol` `\delrow` `\delcol` も)。トップレベル: **数式の改行**(複数行数式。行の間に `┈` 1文字の区切り行を挟んで縦に並ぶ。align 相当の桁揃えはなし) |
| `Backspace` | 中身のある構造には踏み込み、空の構造は削除 |
| `Shift+←` `→` | ブロック選択(兄弟ノード範囲、背景色でハイライト)。選択中に `\frac` `\sqrt` `^` `_` `(` で包む、`Backspace` でまとめて削除 |
| `Shift+↑` | 選択を親構造へ拡大(今いるインセット全体 → さらに外側 → 数式全体) |
| `Ctrl+B` | ブロック選択モード: カーソルの**祖先チェーン**をグラデーション背景で表示。↑/→ で外側、↓/← で内側へ、Enter でそのブロックを丸ごと選択(そのまま ^C/^X・包み込み・削除へ)。もう一度 ^B で解除 |
| `Ctrl+C` `Ctrl+X` `Ctrl+V` | 選択のコピー / カット / ペースト(カット+移動先でペースト=移動) |
| `Ctrl+F` | フリーカーソルモード: 矢印でセル単位に自由移動し、自由カーソル(通常カーソルと同じ反転表示)とスナップ先(最寄りの編集可能位置、色付きセル)を両方表示。未展開の ² や ∑ の空スロットは近づくと自動展開(離れると畳む、ヒステリシス付き)。Enter でスナップ先に移動、Esc でキャンセル |
| `Ctrl+Y` | 正準AAをクリップボードへコピー |
| `Ctrl+T` | イタリック表示切替 |
| `Esc` | モード解除 → 選択解除。解除するものが無ければ**終了**(`Ctrl+Q` も終了だが、端末に取られる環境がある) |

グリッド内・デリミタ内ではヘルプ行が状況依存のヒント(行列の増減コマンド、
`\mid` など)に切り替わります。

## コマンド(抜粋)

- 構造: `\frac` `\sqrt` `\cbrt`(∛)`\qdrt`(∜) `\matrix`(2×2。`\matrix34` で 3行×4列、
  各 *matrix/cases/array 共通。Enter/\addcol などで後から増減も可)
- デリミタ: `\pmatrix` `\Bmatrix` `\vmatrix` `\array`(裸グリッド)`\cases` `\rcases`、`\ceil` `\floor` `\norm`
  `\abs` `\langle` `\braket`(⟨·|·⟩)`\set`({·|·})`\mid`(セグメント分割)
  `\lr\langle||\rangle` = ⟨·|·|·⟩。1文字スペック `()[]{}<>|.` と
  `\langle` 等の名前を混在可、`.` は片側なし)
- 大型演算子: `\sum` `\prod` `\int` `\oint` `\bigcup` …(挿入直後は下極限、
  `↑` で上極限へ)。`\lim` `\max` `\inf` `\det` `\Pr` なども同じバンドで
  下極限に入る(`┈lim┈`)。`\argmax` `\argmin` `\limsup` `\liminf` も
  同じ1語のバンド(`┈argmax┈` — LaTeX では `\operatorname*{arg\,max}`)
- 関数名: `\sin` `\cos` `\log` `\lim` …(立体で表示)。任意名は
  `\op` / `\op*`(別名 `\limits`)— その場に名前入力ボックスが開き、確定でそのまま
  立体ラン(2文字以上は \operatorname)/ 演算子バンド(=\operatorname*、下極限へ)に
  なる(バンド名は1語 — `ess sup` は `┈esssup┈` に連結される)。Esc で取消、
  名前以外のキーはそのまま確定して通常動作
- アクセント: `\hat` `\vec` `\bar` `\dot` `\tilde` `\underline` `\utilde`
  (直前の1文字に付く。続けて実行すると縦に重ね掛け。**選択を包むと
  伸縮アクセント** — 基底は裸のまま、`┈┈˰┈┈` `┈┈￫┈┈` `┈___┈` の
  ようなマーク入り範囲行が上下に付く \widehat 形。分数など背の高い
  選択も可)
- 伸縮矢印: `\xto` `\xfrom`(→ ←)、`\xTo` `\xFrom`(⇒ ⇐)— 上下にラベル
  (`\xrightarrow` などフルネームも可)
- テキスト: `\rmdx` → `dx`(裸の立体ラン。単独1文字は `'d'` = \mathrm)、
  `"` キーでテキストモード(閉じ `"` で確定、`\"` でエスケープ)、
  `\text...` → `"…"`(\text。空白入りは AA 上 ␣)
- ブレース: `\overbrace` `\underbrace`(選択があれば選択を引数にしてラベルへ)
- 記号: 手書きの厳選テーブル(KaTeX/MathJax が描けるコマンドを基準に約450綴り。
  `\bbR`→ℝ, `\->`→→, `\oo`→∞ など)+ スタイル族は規則表に畳んである
  (`\frakA`=`\Afrk`)

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
  `Ctrl+Enter` で書き戻し。選択 AA の LaTeX 変換コマンドも。
- **Obsidian**: コマンド「Edit mascii formula at cursor」でモーダル編集。
- **Zed**: 拡張 UI API 未提供のため CLI タスク連携(選択→aa2tex)。

ビルド手順と「数式部分だけその場で構造編集」への段階的ロードマップは
`docs/editors.md` 参照。

## 設計

内部表現は TeX 文字列ではなく**数式 AST**。構造編集・AA 描画・LaTeX
出力のすべてが AST から導出されます。AA はソースコードで、正準形の
ラウンドトリップ契約(`parse(render(normalize(x))) ==
normalize(strip_spacers(normalize(x)))`)を実式コーパスとランダム生成
2000 ケースのプロパティテストで保証しています(`fmt` が受理形を正準形に
整えます)。

- `docs/aa-spec.md` — 正準AA形式の仕様(予約グリフ・レイアウト)
- `docs/parse-model.md` — パースモデル仕様(読み手視点: 基線復元→走査→再帰)
- `docs/design.md` — 設計判断の経緯とロードマップ
- `CLAUDE.md` — 開発を引き継ぐ AI/人間向けのガイド

```
src/ast.rs      AST・カーソルパス・正規形
src/render.rs   AST → 2D 文字ブロック(正準AA)
src/parse.rs    AA → AST(逆変換)
src/editor.rs   構造エディタ(LyX 型)
src/input.rs    共有キーマップ(TUI/wasm 共通)
src/output/     AST → LaTeX
src/symbols/    記号・関数・アクセント表(すべて手書きの phf テーブル)
src/main.rs     ratatui TUI + CLI
```

## ロードマップ

docs/design.md 参照。直近: LaTeX パーサ、MathML(var* 系 lim = マークと極限の同時装着は未対応)
(`\overline{x+y}`)、BigOp 基底の自由編集(現状 `\argmax` 等は定型)、
LaTeX パーサ(逆方向)、MathML 出力。
