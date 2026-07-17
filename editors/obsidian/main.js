// mascii Obsidian plugin (prototype): edit AA-format math in ```math
// fenced blocks with the LyX-like structural editor (Rust core via WASM,
// nodejs target loaded through Electron's require).
//
// Install: copy this folder to <vault>/.obsidian/plugins/mascii/ together
// with pkg-node/ built by:
//   cd wasm && wasm-pack build --target nodejs --out-dir ../editors/obsidian/pkg-node

const { Plugin, Modal, Notice } = require('obsidian');
const path = require('path');

const FENCES = ['```math', '```mascii', '~~~math', '~~~mascii'];

function loadWasm(plugin) {
  const base = plugin.app.vault.adapter.basePath;
  const dir = path.join(base, plugin.manifest.dir, 'pkg-node', 'mascii_wasm.js');
  // eslint-disable-next-line global-require
  return require(dir);
}

function findBlock(editor, line) {
  let open = null;
  for (let i = 0; i < editor.lineCount(); i++) {
    const t = editor.getLine(i).trim();
    if (open === null && FENCES.includes(t)) {
      open = i;
    } else if (open !== null && (t === '```' || t === '~~~')) {
      if (open <= line && line <= i) return { start: open, end: i };
      open = null;
    }
  }
  return null;
}

class MasciiModal extends Modal {
  constructor(app, wasm, editor, block) {
    super(app);
    this.wasm = wasm;
    this.editor = editor;
    this.block = block;
  }

  onOpen() {
    const { contentEl } = this;
    contentEl.createEl('div', {
      text: '\\cmd  ^/_ scripts  ( ) insets  Tab exit / Space ␣  ⇧←→ select — Ctrl+Enter: apply, Esc Esc: cancel',
      cls: 'mascii-help',
    });
    this.screen = contentEl.createEl('pre', { cls: 'mascii-screen' });
    this.mini = contentEl.createEl('div', { cls: 'mascii-mini' });
    this.latex = contentEl.createEl('div', { cls: 'mascii-latex' });

    this.ed = new this.wasm.MasciiEditor();
    if (this.block) {
      const lines = [];
      for (let i = this.block.start + 1; i < this.block.end; i++) {
        lines.push(this.editor.getLine(i));
      }
      try {
        this.ed.load(lines.join('\n'));
      } catch (e) {
        new Notice(`mascii: parse error, starting empty (${e.message || e})`);
      }
    }
    this.update();

    this.lastEsc = 0;
    this.keyHandler = (e) => {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        this.apply();
        return;
      }
      if (e.key === 'Escape') {
        const now = Date.now();
        if (now - this.lastEsc < 500) {
          this.close();
          return;
        }
        this.lastEsc = now;
        e.stopPropagation(); // keep the modal open on first Esc
      }
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      this.ed.key(e.key, e.shiftKey);
      this.update();
      e.preventDefault();
      e.stopPropagation();
    };
    this.scope.register([], null, () => false); // let keydown reach us
    contentEl.ownerDocument.addEventListener('keydown', this.keyHandler, true);
  }

  update() {
    this.screen.setText(this.ed.screen());
    const mb = this.ed.minibuffer();
    this.mini.setText(mb === undefined || mb === null ? '' : '\\' + mb + '▌');
    this.latex.setText('LaTeX: ' + this.ed.latex());
  }

  apply() {
    const aa = this.ed.aa();
    if (this.block) {
      this.editor.replaceRange(
        aa + '\n',
        { line: this.block.start + 1, ch: 0 },
        { line: this.block.end, ch: 0 }
      );
    } else {
      const cur = this.editor.getCursor();
      this.editor.replaceRange('```math\n' + aa + '\n```\n', cur);
    }
    this.close();
  }

  onClose() {
    this.contentEl.ownerDocument.removeEventListener('keydown', this.keyHandler, true);
    this.contentEl.empty();
  }
}

module.exports = class MasciiPlugin extends Plugin {
  async onload() {
    this.addCommand({
      id: 'edit-formula',
      name: 'Edit mascii formula at cursor',
      editorCallback: (editor) => {
        let wasm;
        try {
          wasm = loadWasm(this);
        } catch (e) {
          new Notice('mascii: pkg-node not found — see plugin README for build steps');
          return;
        }
        const block = findBlock(editor, editor.getCursor().line);
        new MasciiModal(this.app, wasm, editor, block).open();
      },
    });
  }
};
