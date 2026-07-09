# mascii for Zed

Zed の拡張機構(2025 時点)はテーマ・言語・言語サーバー中心で、
VSCode の webview に相当するカスタム UI パネルをまだ提供していない。
そのため「エディタ上に重ねる構造エディタ」は現状実装できない。

代わりに 2 つの経路を用意している:

## 1. タスクで CLI 変換(すぐ使える)

`.zed/tasks.json` に以下を追加すると、選択した AA を変換できる:

```json
[
  {
    "label": "mascii: AA → LaTeX",
    "command": "printf '%s' \"$ZED_SELECTED_TEXT\" | mascii aa2tex",
    "reveal": "always"
  },
  {
    "label": "mascii: AA → Typst",
    "command": "printf '%s' \"$ZED_SELECTED_TEXT\" | mascii aa2typst",
    "reveal": "always"
  },
  {
    "label": "mascii: 整形(正準AA)",
    "command": "printf '%s' \"$ZED_SELECTED_TEXT\" | mascii fmt",
    "reveal": "always"
  }
]
```

(`cargo install --path .` で mascii CLI を PATH に入れておく)

## 2. ターミナルで TUI

Zed 内蔵ターミナルで `mascii` を起動して数式を作り、Ctrl+S で
保存した LaTeX / 表示中の AA をコピーする。

Zed が拡張 UI(スラッシュコマンド/パネル)を開放したら、
wasm コア(`wasm/`)をそのまま流用して VSCode 版と同等の体験を
実装できる設計になっている。
