# mascii for Obsidian(プロトタイプ)

```math フェンス内の AA 数式を LyX 風構造エディタ(モーダル)で編集する。

## インストール

```sh
cd wasm && wasm-pack build --target nodejs --out-dir ../editors/obsidian/pkg-node
mkdir -p <vault>/.obsidian/plugins/mascii
cp -r editors/obsidian/* <vault>/.obsidian/plugins/mascii/
```

設定 → コミュニティプラグイン で mascii を有効化。

## 使い方

コマンドパレット →「Edit mascii formula at cursor」。カーソルが
```math ブロック内ならその数式を、外なら新規数式をカーソル位置に挿入。
キー操作は TUI と同じ(`\frac`␣ `^` `_` `(` Space ⇧←→ 選択 …)。
Ctrl+Enter で反映、Esc 2回でキャンセル。
