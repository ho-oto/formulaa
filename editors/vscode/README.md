# mascii for VSCode(プロトタイプ)

Markdown などのテキストファイル内の ```math フェンスに入った
AA 形式数式を、LyX 風の構造エディタ(webview)で編集する拡張。

## ビルドとインストール

```sh
# 1. wasm コアをビルド(要 wasm-pack)
cd editors/vscode
npm run build-wasm          # → media/pkg/ に生成

# 2. 開発モードで起動
code --extensionDevelopmentPath="$PWD"
# または F5(Run Extension)/ vsce package で vsix 化
```

## 使い方

- `Ctrl+Alt+M`(mac: `Cmd+Alt+M`): カーソル位置の ```math ブロックを
  エディタで開く。ブロック外なら新規数式を作成してカーソル位置に挿入。
- エディタ内は TUI と同じキー: `\frac`␣ `^` `_` `(` `)` Space ←→↑↓
  Shift+←→ 選択など。
- `Ctrl+Enter` で確定(AA をファイルに書き戻し)。`Esc` 2回でキャンセル。
- コマンドパレット: 「mascii: Convert AA selection to LaTeX」
  (選択した AA を変換してクリップボードへ。要 media/pkg-node:
  `wasm-pack build --target nodejs --out-dir ../editors/vscode/media/pkg-node`)

## 既知の制限

- 数式部分を「その場で」構造編集するインライン体験(virtual document /
  カスタムエディタ)は未実装。docs/editors.md のロードマップ参照。
