# Web デモ

`mascii-demo.html` は単一ファイルの自己完結デモ(wasm・JuliaMono サブセット・
例式をすべて埋め込み)。ブラウザで開くだけで動く。

## ビルド

```sh
# 1. wasm コア
cd wasm && wasm-pack build --target web --out-dir pkg && cd ..

# 2. JuliaMono のサブセット(数式レンジ + ASCII)を woff2 で作る
pip install fonttools brotli
curl -sLO https://github.com/cormullion/juliamono/releases/latest/download/JuliaMono-ttf.tar.gz
tar xzf JuliaMono-ttf.tar.gz JuliaMono-Regular.ttf
python3 -c "
from fontTools.subset import main
import sys
sys.argv = ['subset', 'JuliaMono-Regular.ttf',
  '--unicodes=U+0020-007E,U+00A8-00AF,U+02C6-02DF,U+0300-036F,U+0370-03FF,U+2010-205F,U+2070-209F,U+20D0-20FF,U+2100-214F,U+2190-21FF,U+2200-22FF,U+2300-23FF,U+2500-257F,U+2580-259F,U+25A0-25FF,U+2700-27BF,U+27C0-27EF,U+2A00-2AFF,U+2B00-2BFF,U+1D400-1D7FF',
  '--flavor=woff2', '--output-file=JuliaMono-sub.woff2']
main()"

# 3. 合成
python3 tools/build_web_demo.py --font JuliaMono-sub.woff2
open demo/mascii-demo.html
```

例式は docs/examples.md からビルド時に抽出される(コーパスと常に同期)。
テンプレートは `template.html`(`{{GLUE}}` `{{WASM_B64}}` `{{FONT_B64}}`
`{{EXAMPLES_JSON}}` を置換)。

JuliaMono は OFL ライセンス(同梱サブセットも同ライセンスに従う)。
