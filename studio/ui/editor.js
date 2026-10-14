'use strict';
/* NEXUL Studio — the code editor.
 *
 * A transparent <textarea> receives the input (native caret, selection, IME, clipboard);
 * the highlighted text, decorations, gutter and minimap are drawn beneath it, for the
 * visible lines only. The undo history is kept per document by the editor itself: the
 * browser's own history is shared by every field of the page.
 */

const ESC_MAP = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' };
const escHtml = (s) => String(s).replace(/[&<>"]/g, (c) => ESC_MAP[c]);
const escRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
const OPENERS = { '(': ')', '[': ']', '{': '}' };
const CLOSERS = { ')': '(', ']': '[', '}': '{' };
const TAB_W = 4; // width of a tab character on screen (CSS tab-size)

const CLS_VAR = {
  kw: '--s-kw', ctl: '--s-ctl', type: '--s-type', fn: '--s-fn', fndef: '--s-fndef', str: '--s-str', esc: '--s-esc', interp: '--s-interp',
  num: '--s-num', const: '--s-const', comment: '--s-comment', attr: '--s-attr', op: '--s-op', punct: '--s-punct', self: '--s-self',
  tag: '--s-tag', attrname: '--s-attrname', prop: '--s-prop', sel: '--s-sel', entity: '--s-entity', heading: '--s-heading',
  bold: '--fg-strong', b0: '--b0', b1: '--b1', b2: '--b2', ident: '--s-ident',
};

const KIND_BADGE = { kw: ['k-kw', 'K'], fn: ['k-fn', 'ƒ'], type: ['k-type', 'T'], snip: ['k-snip', '{}'], var: ['k-var', 'v'], const: ['k-type', 'C'] };

const CSS_VALUES = ['flex', 'grid', 'block', 'inline-block', 'inline', 'none', 'absolute', 'relative', 'fixed', 'sticky', 'center', 'space-between',
  'space-around', 'flex-start', 'flex-end', 'column', 'row', 'wrap', 'bold', 'normal', 'auto', 'hidden', 'scroll', 'pointer', 'transparent',
  'inherit', 'cover', 'contain', 'solid', 'dashed', 'uppercase', 'italic', 'ease', 'ease-in-out', 'linear', 'rgba()', 'linear-gradient()', 'var()', 'calc()'];
const JS_MEMBERS = ['length', 'push', 'pop', 'map', 'filter', 'forEach', 'reduce', 'find', 'includes', 'indexOf', 'join', 'split', 'slice', 'trim',
  'toUpperCase', 'toLowerCase', 'addEventListener', 'removeEventListener', 'querySelector', 'querySelectorAll', 'classList', 'style', 'textContent',
  'innerHTML', 'value', 'appendChild', 'remove', 'setAttribute', 'getAttribute', 'then', 'catch', 'json', 'log', 'add', 'toggle', 'contains'];

/* Index (UTF-16) of the code point number `cp` of a line. */
function cpIndex(text, cp) {
  let i = 0, k = 0;
  while (k < cp && i < text.length) {
    i += text.codePointAt(i) > 0xffff ? 2 : 1;
    k++;
  }
  return i + Math.max(0, cp - k);
}

function fuzzy(label, p) {
  const l = label.toLowerCase();
  if (l.startsWith(p)) return { score: 100 - label.length * 0.05, hl: [[0, p.length]] };
  const hl = [];
  let k = 0, score = 50, last = -1;
  for (let i = 0; i < l.length && k < p.length; i++) {
    if (l[i] !== p[k]) continue;
    if (k === 0 && i > 0 && !(label[i - 1] === '_' || label[i - 1] === '-' || (label[i] >= 'A' && label[i] <= 'Z'))) continue;
    hl.push([i, i + 1]);
    score -= last >= 0 ? i - last - 1 : i;
    last = i;
    k++;
  }
  return k < p.length ? null : { score, hl };
}

function highlightLabel(label, hl) {
  let out = '', pos = 0;
  for (const [a, b] of hl) {
    out += escHtml(label.slice(pos, a)) + '<b>' + escHtml(label.slice(a, b)) + '</b>';
    pos = b;
  }
  return out + escHtml(label.slice(pos));
}

/* Inline `code` in diagnostic messages. */
function fmtMsg(s) {
  return escHtml(s).replace(/`([^`]+)`/g, '<code>$1</code>');
}

function toHex6(c) {
  if (/^#[0-9a-f]{3,4}$/i.test(c)) return '#' + c[1] + c[1] + c[2] + c[2] + c[3] + c[3];
  if (/^#[0-9a-f]{6}/i.test(c)) return c.slice(0, 7);
  return '#000000';
}

/* The HTML of one line: runs of characters with the same class become one span. */
function renderLine(text, toks, squiggles) {
  const n = text.length;
  if (!n) return '';
  if (n > 4000) return escHtml(text);
  const cls = new Array(n).fill('');
  let ext = null;
  for (const t of toks) {
    const e = Math.min(t[1], n);
    if (t[3] && !ext) ext = new Array(n).fill('');
    for (let k = Math.max(0, t[0]); k < e; k++) {
      cls[k] = t[2];
      if (ext) ext[k] = t[3] || '';
    }
  }
  let sq = null;
  if (squiggles) {
    sq = new Array(n).fill('');
    for (const r of squiggles) for (let k = Math.max(0, r.s); k < Math.min(r.e, n); k++) sq[k] = r.err ? 'sq-err' : 'sq-warn';
  }
  let out = '', k = 0;
  while (k < n) {
    const c = cls[k], x = ext ? ext[k] : '', q = sq ? sq[k] : '';
    let j = k + 1;
    while (j < n && cls[j] === c && (!ext || ext[j] === x) && (!sq || sq[j] === q)) j++;
    const piece = escHtml(text.slice(k, j));
    let classes = c && c !== 'ident' ? 't-' + c : '';
    if (x) classes += ' t-color';
    if (q) classes += ' ' + q;
    classes = classes.trim();
    out += classes ? `<span class="${classes}"${x ? ` style="--swatch:${x}"` : ''}>${piece}</span>` : piece;
    k = j;
  }
  return out;
}

const FIND_WIDGET_HTML = `
<div class="find-widget">
  <div class="find-row">
    <button class="icon-btn fw-toggle-replace" title="Afficher le remplacement">›</button>
    <div class="field"><input class="fw-find" placeholder="Rechercher" spellcheck="false">
      <button class="toggle fw-case" title="Respecter la casse">Aa</button>
      <button class="toggle fw-word" title="Mot entier">ab</button>
      <button class="toggle fw-regex" title="Expression régulière">.*</button></div>
    <span class="find-count"></span>
    <button class="icon-btn fw-prev" title="Précédent (Maj+Entrée)">↑</button>
    <button class="icon-btn fw-next" title="Suivant (Entrée)">↓</button>
    <button class="icon-btn fw-close" title="Fermer (Échap)">✕</button>
  </div>
  <div class="find-row fw-replace-row" style="display:none">
    <span style="width:26px;flex:none"></span>
    <div class="field"><input class="fw-replace" placeholder="Remplacer par" spellcheck="false"></div>
    <button class="btn fw-rep-one" title="Remplacer (Entrée)">Remplacer</button>
    <button class="btn fw-rep-all" title="Tout remplacer (Ctrl+Alt+Entrée)">Tout</button>
  </div>
</div>`;

class Editor {
  static config = { tabSize: 4, minimap: true, wordHighlight: true, autoClose: true };
  static metrics = { cw: 8.4, lh: 21 };
  static colors = {};

  /* Character width and line height of the code font. */
  static measure() {
    const probe = document.createElement('div');
    probe.className = 'ed-text';
    probe.style.cssText = 'visibility:hidden;position:absolute;left:-9999px;top:0';
    probe.textContent = 'x'.repeat(200);
    document.body.appendChild(probe);
    const w = probe.getBoundingClientRect().width;
    Editor.metrics = { cw: w / 200 || 8.4, lh: parseFloat(getComputedStyle(probe).lineHeight) || 21 };
    probe.remove();
  }

  static refreshColors() {
    const cs = getComputedStyle(document.documentElement);
    const c = {};
    for (const k in CLS_VAR) c[k] = cs.getPropertyValue(CLS_VAR[k]).trim() || '#888';
    Editor.colors = c;
    Editor.colorEpoch = (Editor.colorEpoch || 0) + 1;
  }

  constructor(doc, hooks) {
    this.path = doc.path;
    this.lang = doc.lang;
    this.hooks = hooks || {};
    this.eol = doc.text.includes('\r\n') ? '\r\n' : '\n';
    this.cache = [];
    this.valid = 0;
    this.diags = [];
    this.diagLines = new Map();
    this.diagVersion = 0;
    this.version = 0;
    this.lines = [''];
    this.starts = [0];
    this.raf = 0;
    this.hidden = true;
    this.snippet = null;
    this.cmp = null;
    this.undoStack = [];
    this.redoStack = [];
    this.find = { query: '', caseSens: false, word: false, regex: false, matches: [], current: -1, error: false };
    this.fwOpen = false;
    this.build();
    const text = doc.text.replace(/\r\n?/g, '\n');
    this.ta.value = text;
    this.ta.setSelectionRange(0, 0);
    this.value = this.ta.value;
    this.savedValue = this.value;
    this.updateModel(null);
  }

  /* ---------------------------------------------------------------- DOM */

  build() {
    const el = document.createElement('div');
    el.className = 'ed hidden';
    el.innerHTML =
      '<div class="ed-gutter"><div class="ed-gutter-inner"></div></div>' +
      '<div class="ed-view">' +
      '<div class="ed-layer"><div class="ed-decos"></div><div class="ed-text"></div></div>' +
      '<textarea class="ed-input" spellcheck="false" wrap="off" autocomplete="off" autocapitalize="off" autocorrect="off" aria-label="Éditeur de code"></textarea>' +
      '<div class="color-pick" title="Choisir une couleur"><input type="color" tabindex="-1"></div>' +
      FIND_WIDGET_HTML +
      '</div>' +
      '<div class="ed-minimap"><canvas></canvas><div class="mm-slider"></div></div>';
    this.el = el;
    const q = (s) => el.querySelector(s);
    this.gutter = q('.ed-gutter');
    this.gutterInner = q('.ed-gutter-inner');
    this.view = q('.ed-view');
    this.layer = q('.ed-layer');
    this.decoEl = q('.ed-decos');
    this.textEl = q('.ed-text');
    this.ta = q('.ed-input');
    this.picker = q('.color-pick');
    this.pickerInput = q('.color-pick input');
    this.fw = q('.find-widget');
    this.fwFind = q('.fw-find');
    this.fwRep = q('.fw-replace');
    this.fwCount = q('.find-count');
    this.mm = q('.ed-minimap');
    this.mmCanvas = q('.ed-minimap canvas');
    this.mmSlider = q('.mm-slider');

    const ta = this.ta;
    ta.addEventListener('beforeinput', (e) => {
      if (e.inputType === 'historyUndo') { e.preventDefault(); this.undo(); return; }
      if (e.inputType === 'historyRedo') { e.preventDefault(); this.redo(); return; }
      this.selBefore = [ta.selectionStart, ta.selectionEnd];
    });
    ta.addEventListener('input', (e) => this.onNativeInput(e));
    ta.addEventListener('keydown', (e) => this.onKey(e));
    ta.addEventListener('scroll', () => { this.hideHover(); this.schedule(); });
    const sel = () => this.onSelect();
    ta.addEventListener('keyup', sel);
    ta.addEventListener('mouseup', sel);
    ta.addEventListener('focus', sel);
    ta.addEventListener('mousedown', () => this.closeCompletion());
    this.onSelChange = () => { if (document.activeElement === ta) this.onSelect(); };
    document.addEventListener('selectionchange', this.onSelChange);
    ta.addEventListener('blur', () => {
      setTimeout(() => { if (document.activeElement !== ta) this.closeCompletion(); }, 150);
      this.schedule();
    });
    ta.addEventListener('mousemove', (e) => this.onHover(e));
    ta.addEventListener('mouseleave', () => this.hideHoverSoon());
    ta.addEventListener('contextmenu', (e) => {
      if (this.hooks.contextmenu) { e.preventDefault(); this.hooks.contextmenu(e, this); }
    });

    // Gutter: click selects the line; hover shows the diagnostics of the line.
    this.gutter.addEventListener('mousedown', (e) => {
      e.preventDefault();
      const l = this.lineAtY(e.clientY);
      const a = this.starts[l], b = l + 1 < this.lines.length ? this.starts[l + 1] : a + this.lines[l].length;
      ta.focus({ preventScroll: true });
      ta.setSelectionRange(a, b);
      this.onSelect();
    });
    this.gutter.addEventListener('mousemove', (e) => {
      const arr = this.diagLines.get(this.lineAtY(e.clientY));
      if (!arr) return this.hideHoverSoon();
      clearTimeout(this.hoverTimer);
      const d = arr[arr.length - 1].d;
      if (this.hoverDiag === d && this.tipEl) { clearTimeout(this.hideTimer); return; }
      this.hoverTimer = setTimeout(() => this.showHover(d, e.clientX, e.clientY), 200);
    });
    this.gutter.addEventListener('mouseleave', () => this.hideHoverSoon());
    const wheel = (e) => { ta.scrollTop += e.deltaY; ta.scrollLeft += e.deltaX; };
    this.gutter.addEventListener('wheel', wheel, { passive: true });
    this.mm.addEventListener('wheel', wheel, { passive: true });

    // Minimap: click to jump, drag to scroll.
    this.mm.addEventListener('mousedown', (e) => {
      e.preventDefault();
      const { lh } = Editor.metrics;
      const rect = this.mm.getBoundingClientRect();
      if (e.target !== this.mmSlider) {
        const line = (e.clientY - rect.top + (this.mmOff || 0)) / 2;
        ta.scrollTop = line * lh - ta.clientHeight / 2;
      }
      const y0 = e.clientY, s0 = ta.scrollTop;
      const total = this.lines.length * 2, H = this.mm.clientHeight;
      const maxScroll = Math.max(1, ta.scrollHeight - ta.clientHeight);
      let ratio = 2 / lh - (total > H ? (total - H) / maxScroll : 0);
      if (ratio <= 0.0001) ratio = H / maxScroll;
      const move = (ev) => { ta.scrollTop = s0 + (ev.clientY - y0) / ratio; };
      const up = () => { removeEventListener('mousemove', move); removeEventListener('mouseup', up); document.body.classList.remove('resizing'); };
      document.body.classList.add('resizing');
      addEventListener('mousemove', move);
      addEventListener('mouseup', up);
    });

    // Colour picker for colours in CSS.
    this.pickerInput.addEventListener('change', () => {
      const p = this.pick;
      if (!p) return;
      const orig = this.lines[p.l].slice(p.s, p.e);
      let v = this.pickerInput.value;
      if (/^#[0-9a-f]{8}$/i.test(orig)) v += orig.slice(7);
      else if (/^#[0-9a-f]{4}$/i.test(orig)) v += orig[4] + orig[4];
      if (/^#[0-9A-F]+$/.test(orig) && /[A-F]/.test(orig)) v = v.toUpperCase();
      const a = this.starts[p.l] + p.s;
      this.replace(a, a + orig.length, v, a + v.length);
      ta.focus({ preventScroll: true });
    });

    this.buildFind();
    this.ro = new ResizeObserver(() => { this.mmKey = ''; this.schedule(); });
    this.ro.observe(this.view);
  }

  buildFind() {
    const fw = this.fw;
    const q = (s) => fw.querySelector(s);
    this.fwFind.addEventListener('input', () => this.computeMatches(true));
    this.fwFind.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') { e.preventDefault(); this.findNext(e.shiftKey ? -1 : 1); }
      else if (e.key === 'Escape') { e.preventDefault(); this.closeFind(); }
      else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'h') { e.preventDefault(); this.showReplace(true); }
      else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') { e.preventDefault(); this.fwFind.select(); }
    });
    this.fwRep.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') { e.preventDefault(); if (e.ctrlKey && e.altKey) this.replaceAll(); else this.replaceOne(); }
      else if (e.key === 'Escape') { e.preventDefault(); this.closeFind(); }
    });
    const toggle = (sel, key) => q(sel).addEventListener('click', (e) => {
      this.find[key] = !this.find[key];
      e.currentTarget.classList.toggle('on', this.find[key]);
      this.computeMatches(false);
      this.fwFind.focus();
    });
    toggle('.fw-case', 'caseSens');
    toggle('.fw-word', 'word');
    toggle('.fw-regex', 'regex');
    q('.fw-prev').addEventListener('click', () => this.findNext(-1));
    q('.fw-next').addEventListener('click', () => this.findNext(1));
    q('.fw-close').addEventListener('click', () => this.closeFind());
    q('.fw-toggle-replace').addEventListener('click', () => this.showReplace(q('.fw-replace-row').style.display === 'none'));
    q('.fw-rep-one').addEventListener('click', () => this.replaceOne());
    q('.fw-rep-all').addEventListener('click', () => this.replaceAll());
  }

  dispose() {
    document.removeEventListener('selectionchange', this.onSelChange);
    this.ro.disconnect();
    this.closeCompletion();
    this.hideHover();
    cancelAnimationFrame(this.raf);
    this.el.remove();
  }

  show() {
    this.hidden = false;
    this.el.classList.remove('hidden');
    if (this.savedScroll) {
      this.ta.scrollTop = this.savedScroll[0];
      this.ta.scrollLeft = this.savedScroll[1];
    }
    this.tkey = '';
    this.mmKey = '';
    this.schedule();
  }

  hide() {
    this.savedScroll = [this.ta.scrollTop, this.ta.scrollLeft];
    this.hidden = true;
    this.el.classList.add('hidden');
    this.closeCompletion();
    this.hideHover();
  }

  focus() { this.ta.focus({ preventScroll: true }); }

  /* ---------------------------------------------------------------- model */

  getText() { return this.eol === '\r\n' ? this.value.replace(/\n/g, '\r\n') : this.value; }
  isDirty() { return this.value !== this.savedValue; }
  markSaved() { this.savedValue = this.value; }

  /* Replaces the whole text (file changed on disk); the change can be undone. */
  setText(text) {
    const v = text.replace(/\r\n?/g, '\n');
    const s = this.ta.selectionStart;
    this.replace(0, this.value.length, v, Math.min(s, v.length));
    this.savedValue = this.value;
  }

  setLang(lang, path) {
    this.lang = lang;
    this.path = path;
    this.cache = [];
    this.valid = 0;
    this.version++;
    this.tkey = '';
    this.schedule();
  }

  updateModel(old) {
    const lines = this.value.split('\n');
    if (old) {
      const n = Math.min(old.length, lines.length);
      let p = 0;
      while (p < n && old[p] === lines[p]) p++;
      let s = 0;
      while (s < n - p && old[old.length - 1 - s] === lines[lines.length - 1 - s]) s++;
      const removed = old.length - p - s, added = lines.length - p - s;
      if (this.cache.length > p) this.cache = this.cache.slice(0, p).concat(new Array(added), this.cache.slice(p + removed));
      this.valid = Math.min(this.valid, p);
      // Diagnostics after the edited lines move with the text.
      const delta = added - removed, endOld = old.length - s;
      if (this.diags.length && delta) {
        for (const d of this.diags) {
          if (d.l >= endOld) d.l += delta;
          if (d.el >= endOld) d.el += delta;
        }
      }
      this.lines = lines;
      if (this.diags.length) this.indexDiags();
    }
    this.lines = lines;
    const starts = new Array(lines.length);
    let acc = 0;
    for (let i = 0; i < lines.length; i++) { starts[i] = acc; acc += lines[i].length + 1; }
    this.starts = starts;
    this.version++;
  }

  lineOf(off) {
    const s = this.starts;
    let lo = 0, hi = s.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (s[mid] <= off) lo = mid; else hi = mid - 1;
    }
    return lo;
  }

  offset(line, col) {
    const l = clamp(line, 0, this.lines.length - 1);
    return this.starts[l] + clamp(col, 0, this.lines[l].length);
  }

  caret() { return this.ta.selectionDirection === 'backward' ? this.ta.selectionStart : this.ta.selectionEnd; }

  cursorInfo() {
    const c = this.caret(), l = this.lineOf(c);
    const t = this.lines[l], col = c - this.starts[l];
    return { line: l + 1, col: this.vcol(t, col) + 1, sel: Math.abs(this.ta.selectionEnd - this.ta.selectionStart) };
  }

  selectedText() { return this.value.slice(this.ta.selectionStart, this.ta.selectionEnd); }

  vcol(t, idx) {
    if (t.indexOf('\t') < 0) return idx;
    let c = 0;
    for (let k = 0; k < idx && k < t.length; k++) c = t[k] === '\t' ? c + TAB_W - (c % TAB_W) : c + 1;
    return c + Math.max(0, idx - t.length);
  }

  lineAtY(y) {
    const r = this.view.getBoundingClientRect();
    return clamp(Math.floor((y - r.top + this.ta.scrollTop) / Editor.metrics.lh), 0, this.lines.length - 1);
  }

  /* ---------------------------------------------------------------- tokens */

  toks(i) {
    if (i < 0 || i >= this.lines.length) return [];
    const tk = TOKENIZERS[this.lang] || TOKENIZERS.text;
    while (this.valid <= i) {
      const j = this.valid;
      const inSt = j === 0 ? tk.init : this.cache[j - 1].out;
      const key = JSON.stringify(inSt);
      const text = this.lines[j];
      const c = this.cache[j];
      if (!c || c.text !== text || c.key !== key) {
        let r;
        try { r = text.length > 4000 ? { toks: [], st: inSt } : tk.fn(text, inSt); } catch (err) { r = { toks: [], st: inSt }; }
        this.cache[j] = { text, key, toks: r.toks, out: r.st, html: null, hv: -1 };
      }
      this.valid++;
    }
    return this.cache[i].toks;
  }

  stateAt(line) {
    const tk = TOKENIZERS[this.lang] || TOKENIZERS.text;
    if (line <= 0) return tk.init;
    this.toks(line - 1);
    return this.cache[line - 1].out;
  }

  tokAt(l, c) {
    for (const t of this.toks(l)) if (t[0] <= c && c < t[1]) return t;
    return null;
  }

  inEmbedded(line) {
    if (this.lang !== 'html') return null;
    const m = this.stateAt(line)[0];
    return m === 5 ? 'js' : m === 6 ? 'css' : null;
  }

  lineHtml(i) {
    this.toks(i);
    const c = this.cache[i];
    if (c.html !== null && c.hv === this.diagVersion) return c.html;
    c.html = renderLine(c.text, c.toks, this.diagLines.get(i));
    c.hv = this.diagVersion;
    return c.html;
  }

  /* ---------------------------------------------------------------- diagnostics */

  setDiagnostics(list) {
    this.diags = list.filter((d) => d.line > 0).map((d) => {
      const l = d.line - 1, el = Math.max((d.end_line || d.line) - 1, l);
      return {
        l, el,
        c: cpIndex(this.lines[l] || '', d.col - 1),
        ec: cpIndex(this.lines[el] || '', (d.end_col || d.col) - 1),
        zero: el === l && (d.end_col || d.col) <= d.col,
        level: d.level, code: d.code, message: d.message, notes: d.notes || [], labels: d.labels || [],
      };
    });
    this.indexDiags();
    this.schedule();
  }

  indexDiags() {
    const m = new Map();
    for (const d of this.diags) {
      if (d.l < 0 || d.l >= this.lines.length) continue;
      const last = clamp(d.el, d.l, Math.min(this.lines.length - 1, d.l + 60));
      for (let l = d.l; l <= last; l++) {
        const t = this.lines[l], len = t.length;
        let s = l === d.l ? d.c : 0;
        let e = l === last ? (last === d.el ? d.ec : len) : len;
        if (d.zero && l === d.l) {
          e = s;
          while (e < len && isIdent(t[e])) e++;
        }
        if (e <= s) e = s + 1;
        if (s >= len) { s = Math.max(0, len - 1); e = len; }
        if (!m.has(l)) m.set(l, []);
        m.get(l).push({ s, e, err: d.level === 'error', d });
      }
    }
    for (const arr of m.values()) arr.sort((a, b) => a.err - b.err);
    this.diagLines = m;
    this.diagVersion++;
  }

  diagAt(l, c) {
    const arr = this.diagLines.get(l);
    if (!arr) return null;
    for (let k = arr.length - 1; k >= 0; k--) if (c >= arr[k].s && c < arr[k].e) return arr[k];
    return null;
  }

  /* ---------------------------------------------------------------- rendering */

  schedule() {
    if (!this.raf && !this.hidden) this.raf = requestAnimationFrame(() => this.render());
  }

  render() {
    this.raf = 0;
    if (this.hidden) return;
    const { cw, lh } = Editor.metrics;
    const ta = this.ta;
    const top = ta.scrollTop, left = ta.scrollLeft, h = ta.clientHeight || this.view.clientHeight;
    const n = this.lines.length;
    const first = Math.max(0, Math.floor(top / lh) - 1);
    const last = Math.min(n - 1, Math.floor((top + h) / lh) + 1);
    const caret = this.caret();
    const cl = this.lineOf(caret);

    const gw = Math.max(52, Math.round(String(n).length * cw + 36));
    if (gw !== this.gw) { this.gw = gw; this.gutter.style.width = gw + 'px'; }

    this.layer.style.transform = `translate(${-left}px, ${-top}px)`;
    const tkey = first + ':' + last + ':' + this.version + ':' + this.diagVersion;
    if (tkey !== this.tkey) {
      this.tkey = tkey;
      let html = '';
      for (let i = first; i <= last; i++) html += '<div class="ln">' + this.lineHtml(i) + '</div>';
      this.textEl.innerHTML = html;
      this.textEl.style.transform = `translateY(${first * lh}px)`;
    }
    const gkey = tkey + ':' + cl;
    if (gkey !== this.gkey) {
      this.gkey = gkey;
      let g = '';
      for (let i = first; i <= last; i++) {
        const marks = this.diagLines.get(i);
        const mark = marks ? `<span class="gmark ${marks.some((m) => m.err) ? 'err' : 'warn'}"></span>` : '';
        g += `<div class="gl${i === cl ? ' active' : ''}">${i + 1}${mark}</div>`;
      }
      this.gutterInner.innerHTML = g;
    }
    this.gutterInner.style.transform = `translateY(${first * lh - top}px)`;

    const ss = ta.selectionStart, se = ta.selectionEnd;
    let d = '';
    if (ss === se) d += `<div class="deco deco-line" style="top:${cl * lh}px;height:${lh}px"></div>`;
    if (this.lang !== 'text' && this.lang !== 'md') d += this.indentGuides(first, last);
    d += this.findDecos(first, last);
    if (ss === se && document.activeElement === ta) {
      if (Editor.config.wordHighlight) d += this.wordDecos(first, last, caret);
      d += this.bracketDecos(caret);
    }
    if (d !== this.dkey) { this.dkey = d; this.decoEl.innerHTML = d; }

    this.updatePicker(cl, caret - this.starts[cl], ss === se);
    if (this.cmp && this.cmpEl) this.positionCompletion();
    this.renderMinimap(first, last);
  }

  box(l, a, b, cls) {
    const { cw, lh } = Editor.metrics;
    const t = this.lines[l];
    const x1 = 10 + this.vcol(t, a) * cw, x2 = 10 + this.vcol(t, b) * cw;
    return `<div class="deco ${cls}" style="top:${l * lh}px;left:${x1}px;width:${Math.max(x2 - x1, 2)}px;height:${lh}px"></div>`;
  }

  indentGuides(first, last) {
    const unit = Editor.config.tabSize;
    const { cw, lh } = Editor.metrics;
    const ind = (t) => {
      let c = 0;
      for (let k = 0; k < t.length; k++) {
        if (t[k] === ' ') c++;
        else if (t[k] === '\t') c += TAB_W - (c % TAB_W);
        else return c;
      }
      return -1;
    };
    let out = '';
    for (let i = first; i <= last; i++) {
      let w = ind(this.lines[i]);
      if (w < 0) {
        let up = 0, dn = 0;
        for (let k = i - 1; k >= Math.max(0, i - 300); k--) { const x = ind(this.lines[k]); if (x >= 0) { up = x; break; } }
        for (let k = i + 1; k < Math.min(this.lines.length, i + 300); k++) { const x = ind(this.lines[k]); if (x >= 0) { dn = x; break; } }
        w = Math.max(up, dn);
      }
      for (let k = 0; k * unit < w; k++) {
        out += `<div class="deco deco-indent" style="top:${i * lh}px;left:${10 + k * unit * cw}px;height:${lh}px"></div>`;
      }
    }
    return out;
  }

  findDecos(first, last) {
    const f = this.find;
    if (!this.fwOpen || !f.matches.length) return '';
    const a = this.starts[first], b = this.starts[last] + this.lines[last].length;
    let lo = 0, hi = f.matches.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (f.matches[mid][1] <= a) lo = mid + 1; else hi = mid;
    }
    let out = '';
    for (let k = lo; k < f.matches.length && f.matches[k][0] <= b && out.length < 300000; k++) {
      const [s, e] = f.matches[k];
      const l = this.lineOf(s), c = s - this.starts[l];
      const ec = Math.min(e - this.starts[l], this.lines[l].length);
      out += this.box(l, c, Math.max(ec, c + 1), 'deco-find' + (k === f.current ? ' current' : ''));
    }
    return out;
  }

  wordDecos(first, last, caret) {
    if (this.lang === 'text' || this.lang === 'md') return '';
    const l = this.lineOf(caret), t = this.lines[l], c = caret - this.starts[l];
    let s = c, e = c;
    while (s > 0 && isIdent(t[s - 1])) s--;
    while (e < t.length && isIdent(t[e])) e++;
    if (e - s < 2 || isDigit(t[s])) return '';
    const tok = this.tokAt(l, s);
    if (tok && (tok[2] === 'kw' || tok[2] === 'ctl' || tok[2] === 'comment' || tok[2] === 'str')) return '';
    const w = t.slice(s, e);
    const re = new RegExp('(?<![\\w$])' + escRe(w) + '(?![\\w$])', 'g');
    let out = '', count = 0;
    for (let i = first; i <= last; i++) {
      const lt = this.lines[i];
      if (lt.length > 3000) continue;
      re.lastIndex = 0;
      let m;
      while ((m = re.exec(lt))) {
        out += this.box(i, m.index, m.index + w.length, 'deco-word');
        if (++count > 300) return out;
      }
    }
    return count > 1 ? out : '';
  }

  isBracketTok(t) { return t[2].length === 2 && t[2][0] === 'b' && t[1] - t[0] === 1; }

  /* The bracket matching the one at (l, col), found through the tokens (so that brackets
   * in strings and comments are ignored). */
  findMatch(l, col, ch) {
    const fwd = ch in OPENERS;
    let depth = 0;
    if (fwd) {
      for (let i = l; i < this.lines.length && i < l + 3000; i++) {
        for (const t of this.toks(i)) {
          if ((i === l && t[0] <= col) || !this.isBracketTok(t)) continue;
          const x = this.lines[i][t[0]];
          if (x in OPENERS) depth++;
          else if (depth === 0) return OPENERS[ch] === x ? [i, t[0]] : null;
          else depth--;
        }
      }
    } else {
      for (let i = l; i >= 0 && i > l - 3000; i--) {
        const toks = this.toks(i);
        for (let k = toks.length - 1; k >= 0; k--) {
          const t = toks[k];
          if ((i === l && t[0] >= col) || !this.isBracketTok(t)) continue;
          const x = this.lines[i][t[0]];
          if (x in CLOSERS) depth++;
          else if (depth === 0) return CLOSERS[ch] === x ? [i, t[0]] : null;
          else depth--;
        }
      }
    }
    return null;
  }

  bracketDecos(caret) {
    const l = this.lineOf(caret), c = caret - this.starts[l], t = this.lines[l];
    for (const k of [c - 1, c]) {
      if (k < 0 || k >= t.length || !(t[k] in OPENERS || t[k] in CLOSERS)) continue;
      const tok = this.tokAt(l, k);
      if (!tok || !this.isBracketTok(tok)) continue;
      const m = this.findMatch(l, k, t[k]);
      if (!m) return '';
      return this.box(l, k, k + 1, 'deco-bracket') + this.box(m[0], m[1], m[1] + 1, 'deco-bracket');
    }
    return '';
  }

  updatePicker(l, c, collapsed) {
    const tok = collapsed ? this.toks(l).find((t) => t[3] && t[0] <= c && c <= t[1]) : null;
    if (!tok) {
      if (this.pick) { this.picker.classList.remove('open'); this.pick = null; }
      return;
    }
    const { cw, lh } = Editor.metrics;
    const t = this.lines[l];
    const x = 10 + this.vcol(t, t.length) * cw - this.ta.scrollLeft + 10;
    const y = l * lh - this.ta.scrollTop + (lh - 18) / 2;
    this.picker.style.left = x + 'px';
    this.picker.style.top = y + 'px';
    this.picker.style.background = tok[3];
    this.picker.classList.add('open');
    const key = l + ':' + tok[0] + ':' + tok[3];
    if (!this.pick || this.pick.key !== key) this.pickerInput.value = toHex6(tok[3]);
    this.pick = { key, l, s: tok[0], e: tok[1] };
  }

  renderMinimap(first, last) {
    const show = Editor.config.minimap;
    this.mm.classList.toggle('hidden', !show);
    if (!show) return;
    const W = this.mm.clientWidth, H = this.mm.clientHeight;
    if (!W || !H) return;
    const ta = this.ta, LH = 2, CW = 0.75;
    const n = this.lines.length, total = n * LH;
    const maxScroll = Math.max(1, ta.scrollHeight - ta.clientHeight);
    const off = total > H ? Math.min(1, ta.scrollTop / maxScroll) * (total - H) : 0;
    this.mmOff = off;
    const { lh } = Editor.metrics;
    this.mmSlider.style.top = (ta.scrollTop / lh) * LH - off + 'px';
    this.mmSlider.style.height = Math.max(8, (ta.clientHeight / lh) * LH) + 'px';
    const from = Math.floor(off / LH), to = Math.min(n - 1, Math.ceil((off + H) / LH));
    const key = [this.version, from, to, W, H, Editor.colorEpoch, this.lang].join(':');
    if (key === this.mmKey) return;
    this.mmKey = key;
    const dpr = window.devicePixelRatio || 1;
    const cv = this.mmCanvas;
    if (cv.width !== Math.round(W * dpr) || cv.height !== Math.round(H * dpr)) {
      cv.width = Math.round(W * dpr);
      cv.height = Math.round(H * dpr);
      cv.style.width = W + 'px';
      cv.style.height = H + 'px';
    }
    const ctx = cv.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    const colors = Editor.colors;
    const maxCol = Math.floor((W - 6) / CW);
    const runs = (t, a, b, y) => {
      let k = a;
      while (k < b) {
        while (k < b && (t[k] === ' ' || t[k] === '\t')) k++;
        const s = k;
        while (k < b && t[k] !== ' ' && t[k] !== '\t') k++;
        if (k > s) {
          const x1 = this.vcol(t, s), x2 = Math.min(this.vcol(t, k), maxCol);
          if (x1 < maxCol) ctx.fillRect(4 + x1 * CW, y, (x2 - x1) * CW, LH - 0.5);
        }
      }
    };
    for (let i = from; i <= to; i++) {
      const t = this.lines[i];
      if (!t) continue;
      const y = i * LH - off;
      const len = Math.min(t.length, 400);
      let pos = 0;
      for (const tok of this.toks(i)) {
        if (tok[0] >= len) break;
        if (tok[0] > pos) { ctx.globalAlpha = 0.5; ctx.fillStyle = colors.ident; runs(t, pos, tok[0], y); }
        ctx.globalAlpha = 0.85;
        ctx.fillStyle = colors[tok[2]] || colors.ident;
        runs(t, tok[0], Math.min(tok[1], len), y);
        pos = Math.max(pos, tok[1]);
      }
      if (pos < len) { ctx.globalAlpha = 0.5; ctx.fillStyle = colors.ident; runs(t, pos, len, y); }
    }
    ctx.globalAlpha = 1;
    for (const [l, arr] of this.diagLines) {
      if (l < from || l > to) continue;
      ctx.fillStyle = arr.some((a) => a.err) ? getComputedStyle(this.el).getPropertyValue('--error') : getComputedStyle(this.el).getPropertyValue('--warning');
      ctx.fillRect(W - 4, l * LH - off - 1, 4, 4);
    }
  }

  /* ---------------------------------------------------------------- editing primitives */

  /* Replaces [from, to) by `text` and selects [a, b) (the end of the insertion by default). */
  replace(from, to, text, a, b) {
    const ta = this.ta;
    const sb = [ta.selectionStart, ta.selectionEnd];
    const del = this.value.slice(from, to);
    if (a === undefined) a = from + text.length;
    if (b === undefined) b = a;
    if (del !== text) {
      ta.setRangeText(text, from, to, 'end');
      this.value = ta.value;
      this.record(from, del, text, sb, [a, b], 'other');
      ta.setSelectionRange(Math.min(a, b), Math.max(a, b), a > b ? 'backward' : 'forward');
      this.afterChange(null);
    } else {
      ta.setSelectionRange(Math.min(a, b), Math.max(a, b));
    }
    this.revealOffset(this.caret());
    this.onSelect();
  }

  record(at, del, ins, sb, sa, kind) {
    if (del === ins) return;
    this.redoStack.length = 0;
    const now = Date.now();
    const last = this.undoStack[this.undoStack.length - 1];
    if (last && now - last.t < 1500) {
      if (kind === 'type' && last.kind === 'type' && !del && !last.del && at === last.at + last.ins.length && !ins.includes('\n') &&
        !(/\s/.test(ins) && !/\s$/.test(last.ins))) {
        last.ins += ins; last.sa = sa; last.t = now;
        return;
      }
      if (kind === 'del' && last.kind === 'del' && !ins && !last.ins && (at + del.length === last.at || at === last.at)) {
        if (at === last.at) last.del += del; else { last.at = at; last.del = del + last.del; }
        last.sa = sa; last.t = now;
        return;
      }
    }
    this.undoStack.push({ at, del, ins, sb, sa, kind, t: now });
    if (this.undoStack.length > 3000) this.undoStack.shift();
  }

  undo() {
    const r = this.undoStack.pop();
    if (!r) return;
    const ta = this.ta;
    ta.setRangeText(r.del, r.at, r.at + r.ins.length, 'end');
    this.value = ta.value;
    r.t = 0;
    this.redoStack.push(r);
    ta.setSelectionRange(r.sb[0], r.sb[1]);
    this.snippet = null;
    this.afterChange(null);
    this.revealOffset(r.sb[0]);
    this.onSelect();
  }

  redo() {
    const r = this.redoStack.pop();
    if (!r) return;
    const ta = this.ta;
    ta.setRangeText(r.ins, r.at, r.at + r.del.length, 'end');
    this.value = ta.value;
    r.t = 0;
    this.undoStack.push(r);
    ta.setSelectionRange(Math.min(r.sa[0], r.sa[1]), Math.max(r.sa[0], r.sa[1]));
    this.afterChange(null);
    this.revealOffset(r.sa[1]);
    this.onSelect();
  }

  onNativeInput(e) {
    const old = this.value, nv = this.ta.value;
    const n = Math.min(old.length, nv.length);
    let p = 0;
    while (p < n && old.charCodeAt(p) === nv.charCodeAt(p)) p++;
    if (this.selBefore) p = Math.min(p, this.selBefore[0]);
    let s = 0;
    while (s < n - p && old.charCodeAt(old.length - 1 - s) === nv.charCodeAt(nv.length - 1 - s)) s++;
    const del = old.slice(p, old.length - s), ins = nv.slice(p, nv.length - s);
    const kind = e.inputType === 'insertText' ? 'type' : e.inputType && e.inputType.startsWith('delete') ? 'del' : 'other';
    this.value = nv;
    this.record(p, del, ins, this.selBefore || [p, p + del.length], [this.ta.selectionStart, this.ta.selectionEnd], kind);
    this.selBefore = null;
    this.afterChange(e);
  }

  afterChange(e) {
    const oldLen = this.starts[this.starts.length - 1] + this.lines[this.lines.length - 1].length;
    this.updateModel(this.lines);
    const delta = this.value.length - oldLen;
    if (this.snippet && delta) {
      const sn = this.snippet;
      sn.stops[sn.i][1] += delta;
      for (let k = sn.i + 1; k < sn.stops.length; k++) { sn.stops[k][0] += delta; sn.stops[k][1] += delta; }
      if (sn.stops[sn.i][1] < sn.stops[sn.i][0]) this.snippet = null;
    }
    if (this.fwOpen && this.find.query) this.computeMatches(false);
    this.hideHover();
    if (this.hooks.change) this.hooks.change(this);
    if (e) {
      const data = e.data;
      if (e.inputType === 'insertText' && data && data.length === 1) {
        const lang = this.inEmbedded(this.lineOf(this.ta.selectionStart)) || this.lang;
        const trigger = isIdent(data) || (data === '.' && (lang === 'nexul' || lang === 'js')) || (data === '<' && lang === 'html') ||
          (data === '-' && (lang === 'css' || lang === 'html') && this.cmp);
        if (trigger) this.openCompletion(false); else this.closeCompletion();
      } else if (e.inputType && e.inputType.startsWith('delete') && this.cmp) {
        this.openCompletion(false);
      } else this.closeCompletion();
    } else if (!this.accepting) {
      this.closeCompletion();
    }
    this.schedule();
  }

  onSelect() {
    const pos = this.caret();
    if (this.snippet) {
      const st = this.snippet.stops[this.snippet.i];
      if (!st || pos < st[0] || pos > st[1]) this.snippet = null;
    }
    if (this.cmp && (this.lineOf(pos) !== this.cmp.line || pos < this.cmp.from)) this.closeCompletion();
    this.schedule();
    if (this.hooks.cursor) this.hooks.cursor(this);
  }

  revealOffset(off, center) {
    const { cw, lh } = Editor.metrics;
    const ta = this.ta;
    const l = this.lineOf(off), c = off - this.starts[l];
    const y = l * lh, h = ta.clientHeight;
    if (center) {
      if (y < ta.scrollTop || y + lh > ta.scrollTop + h) ta.scrollTop = Math.max(0, y - h / 2 + lh / 2);
    } else if (y < ta.scrollTop) ta.scrollTop = y;
    else if (y + lh > ta.scrollTop + h) ta.scrollTop = y + lh - h;
    const x = 10 + this.vcol(this.lines[l], c) * cw, w = ta.clientWidth;
    if (x < ta.scrollLeft + 10) ta.scrollLeft = Math.max(0, x - 40);
    else if (x > ta.scrollLeft + w - 30) ta.scrollLeft = x - w + 60;
    this.schedule();
  }

  goTo(l, c, el, ec) {
    const a = this.offset(l, c);
    const b = el === undefined ? a : this.offset(el, ec);
    this.ta.focus({ preventScroll: true });
    this.ta.setSelectionRange(a, b);
    this.revealOffset(a, true);
    this.onSelect();
  }

  lineRange() {
    const ta = this.ta, s = ta.selectionStart, e = ta.selectionEnd;
    const l1 = this.lineOf(s);
    let l2 = this.lineOf(e);
    if (e > s && l2 > l1 && e === this.starts[l2]) l2--;
    return [l1, l2];
  }

  lineEnd(l) { return this.starts[l] + this.lines[l].length; }

  /* ---------------------------------------------------------------- keyboard */

  onKey(e) {
    if (e.isComposing || e.keyCode === 229) return;
    const k = e.key;
    const ctrl = e.ctrlKey || e.metaKey;
    const altGr = e.ctrlKey && e.altKey;
    const stop = () => { e.preventDefault(); };
    if (this.cmp) {
      if (k === 'ArrowDown') { this.moveCmp(1); return stop(); }
      if (k === 'ArrowUp') { this.moveCmp(-1); return stop(); }
      if (k === 'PageDown') { this.moveCmp(8); return stop(); }
      if (k === 'PageUp') { this.moveCmp(-8); return stop(); }
      if ((k === 'Enter' || k === 'Tab') && !e.shiftKey && !ctrl) { this.acceptCompletion(); return stop(); }
      if (k === 'Escape') { this.closeCompletion(); return stop(); }
    }
    if (k === 'Escape') {
      if (this.snippet) { this.snippet = null; return stop(); }
      if (this.fwOpen) { this.closeFind(); return stop(); }
      return;
    }
    if (ctrl && !e.altKey) {
      const lk = k.toLowerCase();
      if (lk === 'z' && !e.shiftKey) { this.undo(); return stop(); }
      if (lk === 'y' || (lk === 'z' && e.shiftKey)) { this.redo(); return stop(); }
      if (k === ' ') { this.openCompletion(true); return stop(); }
      if (lk === 'f' && !e.shiftKey) { this.openFind(false); return stop(); }
      if (lk === 'h' && !e.shiftKey) { this.openFind(true); return stop(); }
      if (k === '/' || k === ':' || e.code === 'Slash') { this.toggleComment(); return stop(); }
      if (lk === 'k' && e.shiftKey) { this.deleteLines(); return stop(); }
      if (lk === 'l' && !e.shiftKey) { this.selectLines(); return stop(); }
      if (k === 'Enter') { this.newLineAround(e.shiftKey ? -1 : 1); return stop(); }
      if ((lk === 'c' || lk === 'x') && !e.shiftKey && this.ta.selectionStart === this.ta.selectionEnd) {
        this.copyLine(lk === 'x');
        return stop();
      }
      return;
    }
    if (e.altKey && !ctrl && (k === 'ArrowUp' || k === 'ArrowDown')) {
      if (e.shiftKey) this.duplicateLines(k === 'ArrowDown'); else this.moveLines(k === 'ArrowDown' ? 1 : -1);
      return stop();
    }
    if (k === 'F3') { this.findNext(e.shiftKey ? -1 : 1); return stop(); }
    if (k === 'Tab' && !ctrl && !e.altKey) { this.onTab(e.shiftKey); return stop(); }
    if (k === 'Enter' && !ctrl && !e.altKey) { this.onEnter(); return stop(); }
    if (k === 'Backspace' && !ctrl && !e.altKey && !e.shiftKey) { if (this.onBackspace()) stop(); return; }
    if (k === 'Home' && !ctrl && !e.altKey) { this.smartHome(e.shiftKey); return stop(); }
    if (k.length === 1 && (!ctrl || altGr) && !e.metaKey) {
      if (this.cmp && !isIdent(k) && k !== '-') this.closeCompletion();
      if (this.onChar(k)) stop();
    }
  }

  pairsMap() {
    const pairs = (LANGS[this.inEmbedded(this.lineOf(this.ta.selectionStart)) || this.lang] || LANGS.text).pairs;
    const m = {};
    for (let i = 0; i < pairs.length; i += 2) m[pairs[i]] = pairs[i + 1];
    return m;
  }

  onChar(ch) {
    const ta = this.ta;
    const s = ta.selectionStart, e = ta.selectionEnd;
    const line = this.lineOf(s), t = this.lines[line], col = s - this.starts[line];
    const next = t[col] || '', prev = t[col - 1] || '';
    const pairs = this.pairsMap();
    const closers = new Set(Object.values(pairs));
    const auto = Editor.config.autoClose;
    // Type over the closing character inserted automatically.
    if (auto && s === e && closers.has(ch) && next === ch) {
      ta.setSelectionRange(s + 1, s + 1);
      this.onSelect();
      return true;
    }
    if (auto && ch in pairs) {
      const close = pairs[ch];
      if (s !== e) {
        const sel = this.value.slice(s, e);
        this.replace(s, e, ch + sel + close, s + 1, e + 1);
        return true;
      }
      const tok = this.tokAt(line, Math.max(0, col - 1));
      const inComment = tok && tok[2] === 'comment' && col > 0;
      if (inComment) return false;
      if (ch === close) {
        if (isIdent(prev) || prev === ch || isIdent(next)) return false;
        if (tok && tok[2] === 'str' && col > tok[0] && col < tok[1]) return false;
      }
      const nextOk = next === '' || /\s/.test(next) || closers.has(next) || ',;:.'.includes(next);
      if (!nextOk) return false;
      this.replace(s, e, ch + close, s + 1);
      return true;
    }
    if (this.lang === 'html' && s === e && !this.inEmbedded(line)) {
      if (ch === '>') {
        const before = t.slice(0, col);
        const m = /<([a-zA-Z][\w:-]*)(?:\s+(?:[^<>"']|"[^"]*"|'[^']*')*)?$/.exec(before);
        if (m && !before.endsWith('/') && !VOID_TAGS.has(m[1].toLowerCase())) {
          const close = `</${m[1]}>`;
          if (!t.slice(col).startsWith(close)) {
            this.replace(s, e, '>' + close, s + 1);
            return true;
          }
        }
      }
      if (ch === '/' && prev === '<') {
        const tag = this.unclosedTag(s - 1);
        if (tag) {
          // The closing tag is already there (inserted with the opening one): step over it.
          const existing = '</' + tag + '>';
          if (t.slice(col).toLowerCase().startsWith(existing)) {
            this.replace(s - 1, s + existing.length - 1, existing);
            return true;
          }
          const ins = '/' + tag + (next === '>' ? '' : '>');
          const pre = t.slice(0, col - 1);
          if (/^\s*$/.test(pre)) {
            // Align the closing tag with its opening tag.
            const ind = this.openTagIndent(s - 1, tag);
            if (ind !== null && ind !== pre) {
              const ls = this.starts[line];
              this.replace(ls, s, ind + '<' + ins, ls + ind.length + 1 + ins.length + (next === '>' ? 1 : 0));
              return true;
            }
          }
          this.replace(s, e, ins, s + ins.length + (next === '>' ? 1 : 0));
          return true;
        }
      }
    }
    // A closing bracket typed at the start of a line is aligned with its opening line.
    if (s === e && ch in CLOSERS && /^\s+$/.test(t.slice(0, col))) {
      const m = this.findMatch(line, col, ch);
      if (m) {
        const ind = /^[ \t]*/.exec(this.lines[m[0]])[0];
        if (ind !== t.slice(0, col)) {
          const ls = this.starts[line];
          this.replace(ls, s, ind + ch, ls + ind.length + 1);
          return true;
        }
      }
    }
    return false;
  }

  /* The innermost element still open before `pos` (HTML). */
  unclosedTag(pos) {
    const text = this.value.slice(Math.max(0, pos - 100000), pos);
    const re = /<!--[\s\S]*?-->|<(\/?)([a-zA-Z][\w:-]*)((?:[^<>"']|"[^"]*"|'[^']*')*)>/g;
    const stack = [];
    let m;
    while ((m = re.exec(text))) {
      if (!m[2]) continue;
      const name = m[2].toLowerCase();
      if (m[1]) {
        const i = stack.lastIndexOf(name);
        if (i >= 0) stack.length = i;
      } else if (!VOID_TAGS.has(name) && !m[3].trim().endsWith('/')) {
        stack.push(name);
        if (name === 'script' || name === 'style') {
          const end = text.toLowerCase().indexOf('</' + name, re.lastIndex);
          if (end < 0) break;
          re.lastIndex = end;
        }
      }
    }
    return stack.length ? stack[stack.length - 1] : null;
  }

  openTagIndent(pos, tag) {
    const text = this.value.slice(0, pos);
    const re = new RegExp('<' + escRe(tag) + '(?=[\\s>/])', 'gi');
    let m, last = -1;
    while ((m = re.exec(text))) last = m.index;
    if (last < 0) return null;
    return /^[ \t]*/.exec(this.lines[this.lineOf(last)])[0];
  }

  onTab(shift) {
    const ta = this.ta;
    if (this.snippet && !shift) { this.nextStop(); return; }
    const s = ta.selectionStart, e = ta.selectionEnd;
    if (shift || (s !== e && this.value.slice(s, e).includes('\n'))) { this.indentLines(shift); return; }
    if (s === e && (this.lang === 'html' || this.lang === 'md') && this.expandAbbr()) return;
    if (s !== e) { this.indentLines(false); return; }
    const l = this.lineOf(s), unit = Editor.config.tabSize;
    const n = unit - (this.vcol(this.lines[l], s - this.starts[l]) % unit);
    this.replace(s, e, ' '.repeat(n));
  }

  nextStop() {
    const sn = this.snippet;
    sn.i++;
    if (sn.i >= sn.stops.length) { this.snippet = null; return; }
    const [a, b] = sn.stops[sn.i];
    this.ta.setSelectionRange(a, b);
    if (sn.i === sn.stops.length - 1) this.snippet = null;
    this.revealOffset(a);
    this.onSelect();
  }

  /* `!` → HTML skeleton; `tag`, `.class`, `tag.class#id` → element (HTML). */
  expandAbbr() {
    const s = this.ta.selectionStart;
    const l = this.lineOf(s), t = this.lines[l], col = s - this.starts[l];
    const before = t.slice(0, col);
    if (this.lang === 'html' && /^\s*!$/.test(before)) {
      this.insertSnippet(HTML_SKELETON, s - 1, s);
      return true;
    }
    if (this.inEmbedded(l)) return false;
    const m = /(^|[\s>])([a-zA-Z][\w-]*)?((?:[.#][\w-]+)*)$/.exec(before);
    if (!m) return false;
    const tag = (m[2] || '').toLowerCase(), rest = m[3] || '';
    if (!tag && !rest) return false;
    if (tag && !HTML_TAGS.includes(tag)) return false;
    const lt = before.lastIndexOf('<'), gt = before.lastIndexOf('>');
    if (lt > gt) return false;
    const name = tag || 'div';
    const classes = [...rest.matchAll(/\.([\w-]+)/g)].map((x) => x[1]);
    const id = (/#([\w-]+)/.exec(rest) || [])[1];
    let attrs = '';
    if (id) attrs += ` id="${id}"`;
    if (classes.length) attrs += ` class="${classes.join(' ')}"`;
    const start = s - (m[2] || '').length - rest.length;
    let body;
    if (name === 'img') body = '<img src="${1}" alt="${2}"' + attrs + '>';
    else if (name === 'input') body = '<input type="${1:text}"' + attrs + '>';
    else if (name === 'link') body = '<link rel="stylesheet" href="${1:style.css}">';
    else if (name === 'script') body = '<script src="${1:script.js}"></script>';
    else if (name === 'a') body = '<a href="${1:#}"' + attrs + '>$0</a>';
    else if (VOID_TAGS.has(name)) body = '<' + name + attrs + '>';
    else body = '<' + name + attrs + '>$0</' + name + '>';
    this.insertSnippet(body, start, s);
    return true;
  }

  /* Inserts a snippet: `\t` is one indentation level, `${1:text}`, `$1`… are the stops
   * (visited with Tab), `$0` is the final position. */
  insertSnippet(body, from, to) {
    const ta = this.ta;
    if (from === undefined) { from = ta.selectionStart; to = ta.selectionEnd; }
    const l = this.lineOf(from);
    const indent = /^[ \t]*/.exec(this.lines[l])[0];
    const unit = ' '.repeat(Editor.config.tabSize);
    let out = '';
    const stops = new Map();
    let i = 0;
    while (i < body.length) {
      const ch = body[i];
      if (ch === '$') {
        const rest = body.slice(i);
        const m = /^\$\{(\d+):([^}]*)\}/.exec(rest) || /^\$\{(\d+)\}/.exec(rest) || /^\$(\d+)/.exec(rest);
        if (m) {
          const n = +m[1], def = m[2] || '';
          if (!stops.has(n)) stops.set(n, [out.length, out.length + def.length]);
          out += def;
          i += m[0].length;
          continue;
        }
      }
      if (ch === '\n') out += '\n' + indent;
      else if (ch === '\t') out += unit;
      else out += ch;
      i++;
    }
    const order = [...stops.keys()].filter((n) => n > 0).sort((a, b) => a - b);
    const list = order.map((n) => stops.get(n));
    list.push(stops.has(0) ? stops.get(0) : [out.length, out.length]);
    const abs = list.map(([a, b]) => [from + a, from + b]);
    this.accepting = true;
    this.replace(from, to, out, abs[0][0], abs[0][1]);
    this.accepting = false;
    this.snippet = abs.length > 1 ? { stops: abs, i: 0 } : null;
  }

  insertAtCursor(text) {
    const ta = this.ta;
    if (/\$\{?\d/.test(text)) this.insertSnippet(text);
    else this.replace(ta.selectionStart, ta.selectionEnd, text);
    this.focus();
  }

  onEnter() {
    const ta = this.ta;
    const s = ta.selectionStart, e = ta.selectionEnd;
    const l = this.lineOf(s), t = this.lines[l], col = s - this.starts[l];
    const l2 = this.lineOf(e), after = this.lines[l2].slice(e - this.starts[l2]);
    const before = t.slice(0, col);
    const lead = /^[ \t]*/.exec(t)[0];
    const indent = lead.length > before.length ? before : lead;
    const unit = ' '.repeat(Editor.config.tabSize);
    if (this.lang === 'md' && s === e) {
      const m = /^(\s*)([-*+]|(\d+)\.)(\s+)/.exec(t);
      if (m && col >= m[0].length) {
        if (!t.slice(m[0].length).trim()) {
          this.replace(this.starts[l], this.lineEnd(l), '', this.starts[l]);
          return;
        }
        const ins = '\n' + m[1] + (m[3] ? +m[3] + 1 + '.' : m[2]) + ' ';
        this.replace(s, e, ins);
        return;
      }
    }
    const tb = before.replace(/[ \t]+$/, ''), tafter = after.replace(/^[ \t]+/, '');
    const lastCh = tb.slice(-1), firstCh = tafter[0] || '';
    let extra = '', between = false;
    const lang = this.inEmbedded(l) || this.lang;
    if (lastCh in OPENERS && lang !== 'text' && lang !== 'md') {
      extra = unit;
      between = OPENERS[lastCh] === firstCh;
    } else if (lang === 'html') {
      const open = /<([a-zA-Z][\w:-]*)(?:\s(?:[^<>"']|"[^"]*"|'[^']*')*)?>$/.exec(tb);
      if (open && !VOID_TAGS.has(open[1].toLowerCase())) {
        extra = unit;
        between = tafter.toLowerCase().startsWith('</' + open[1].toLowerCase());
      }
    }
    const from = s - (before.length - tb.length), to = e + (after.length - tafter.length);
    let ins = '\n' + indent + extra;
    const caretAt = from + ins.length;
    if (between) ins += '\n' + indent;
    this.replace(from, to, ins, caretAt);
  }

  onBackspace() {
    const ta = this.ta, s = ta.selectionStart;
    if (s !== ta.selectionEnd || s === 0) return false;
    const l = this.lineOf(s), t = this.lines[l], col = s - this.starts[l];
    if (col > 0 && /^ +$/.test(t.slice(0, col))) {
      const n = ((col - 1) % Editor.config.tabSize) + 1;
      this.replace(s - n, s, '', s - n);
      return true;
    }
    const pairs = this.pairsMap();
    if (Editor.config.autoClose && col > 0 && pairs[t[col - 1]] === t[col] && t[col] !== undefined) {
      this.replace(s - 1, s + 1, '', s - 1);
      return true;
    }
    return false;
  }

  smartHome(shift) {
    const ta = this.ta;
    const caret = this.caret(), l = this.lineOf(caret), t = this.lines[l];
    const ind = /^[ \t]*/.exec(t)[0].length, col = caret - this.starts[l];
    const target = this.starts[l] + (col === ind ? 0 : ind);
    if (!shift) ta.setSelectionRange(target, target);
    else {
      const anchor = ta.selectionDirection === 'backward' ? ta.selectionEnd : ta.selectionStart;
      if (target < anchor) ta.setSelectionRange(target, anchor, 'backward');
      else ta.setSelectionRange(anchor, target, 'forward');
    }
    this.revealOffset(target);
    this.onSelect();
  }

  indentLines(out) {
    const ta = this.ta, s = ta.selectionStart, e = ta.selectionEnd;
    const [l1, l2] = this.lineRange();
    const unit = Editor.config.tabSize, pad = ' '.repeat(unit);
    const from = this.starts[l1], to = this.lineEnd(l2);
    const res = [];
    let firstDelta = 0, total = 0;
    for (let i = l1; i <= l2; i++) {
      const t = this.lines[i];
      let nt;
      if (out) {
        let k = 0;
        while (k < unit && t[k] === ' ') k++;
        if (k === 0 && t[0] === '\t') k = 1;
        nt = t.slice(k);
      } else nt = t.length || l1 === l2 ? pad + t : t;
      if (i === l1) firstDelta = nt.length - t.length;
      total += nt.length - t.length;
      res.push(nt);
    }
    const text = res.join('\n');
    if (s === e) {
      const c = Math.max(from, s + firstDelta);
      this.replace(from, to, text, c);
    } else {
      const ns = s === from && !out ? from : Math.max(from, s + firstDelta);
      this.replace(from, to, text, ns, Math.max(ns, e + total));
    }
  }

  commentTokens(line) {
    const emb = this.inEmbedded(line);
    if (emb === 'js') return '//';
    if (emb === 'css') return ['/*', '*/'];
    return (LANGS[this.lang] || {}).comment;
  }

  toggleComment() {
    const ta = this.ta, s = ta.selectionStart, e = ta.selectionEnd;
    const [l1, l2] = this.lineRange();
    const c = this.commentTokens(l1);
    if (!c) return;
    const from = this.starts[l1], to = this.lineEnd(l2);
    const lines = this.lines.slice(l1, l2 + 1);
    let res;
    if (typeof c === 'string') {
      const nonEmpty = lines.filter((t) => t.trim());
      if (!nonEmpty.length) return;
      const all = nonEmpty.every((t) => t.trimStart().startsWith(c));
      if (all) {
        res = lines.map((t) => {
          const k = t.indexOf(c);
          if (k < 0 || t.slice(0, k).trim()) return t;
          return t.slice(0, k) + t.slice(k + c.length + (t[k + c.length] === ' ' ? 1 : 0));
        });
      } else {
        const ind = Math.min(...nonEmpty.map((t) => /^[ \t]*/.exec(t)[0].length));
        res = lines.map((t) => (t.trim() ? t.slice(0, ind) + c + ' ' + t.slice(ind) : t));
      }
    } else {
      const [o, cl] = c;
      const joined = lines.join('\n');
      const tr = joined.trim();
      if (tr.startsWith(o) && tr.endsWith(cl)) {
        const a = joined.indexOf(o), b = joined.lastIndexOf(cl);
        let inner = joined.slice(a + o.length, b);
        if (inner.startsWith(' ')) inner = inner.slice(1);
        if (inner.endsWith(' ')) inner = inner.slice(0, -1);
        res = [joined.slice(0, a) + inner + joined.slice(b + cl.length)];
      } else {
        const ind = /^[ \t]*/.exec(lines[0])[0].length;
        res = [joined.slice(0, ind) + o + ' ' + joined.slice(ind) + ' ' + cl];
      }
    }
    const text = res.join('\n');
    if (s === e) {
      const d0 = text.split('\n')[0].length - lines[0].length;
      this.replace(from, to, text, clamp(s + d0, from, from + text.length));
    } else this.replace(from, to, text, from, from + text.length);
  }

  moveLines(dir) {
    const [l1, l2] = this.lineRange();
    if ((dir < 0 && l1 === 0) || (dir > 0 && l2 >= this.lines.length - 1)) return;
    const ta = this.ta, s = ta.selectionStart, e = ta.selectionEnd;
    const block = this.lines.slice(l1, l2 + 1).join('\n');
    if (dir < 0) {
      const above = this.lines[l1 - 1], d = above.length + 1;
      this.replace(this.starts[l1 - 1], this.lineEnd(l2), block + '\n' + above, s - d, e - d);
    } else {
      const below = this.lines[l2 + 1], d = below.length + 1;
      this.replace(this.starts[l1], this.lineEnd(l2 + 1), below + '\n' + block, s + d, e + d);
    }
  }

  duplicateLines(down) {
    const [l1, l2] = this.lineRange();
    const ta = this.ta, s = ta.selectionStart, e = ta.selectionEnd;
    const block = this.lines.slice(l1, l2 + 1).join('\n');
    const end = this.lineEnd(l2);
    if (down) this.replace(end, end, '\n' + block, s + block.length + 1, e + block.length + 1);
    else this.replace(end, end, '\n' + block, s, e);
  }

  deleteLines() {
    const [l1, l2] = this.lineRange();
    const n = this.lines.length;
    const caret = this.caret(), col = caret - this.starts[this.lineOf(caret)];
    let from = this.starts[l1];
    const to = l2 + 1 < n ? this.starts[l2 + 1] : this.lineEnd(l2);
    if (l2 + 1 >= n && l1 > 0) from--;
    this.replace(from, to, '');
    const nl = Math.min(l1, this.lines.length - 1);
    const off = this.offset(nl, col);
    this.ta.setSelectionRange(off, off);
    this.onSelect();
  }

  selectLines() {
    const [l1, l2] = this.lineRange();
    const end = l2 + 1 < this.lines.length ? this.starts[l2 + 1] : this.lineEnd(l2);
    this.ta.setSelectionRange(this.starts[l1], end);
    this.onSelect();
  }

  newLineAround(dir) {
    const l = this.lineOf(this.caret()), t = this.lines[l];
    let ind = /^[ \t]*/.exec(t)[0];
    if (dir > 0) {
      if (t.trimEnd().slice(-1) in OPENERS) ind += ' '.repeat(Editor.config.tabSize);
      const end = this.lineEnd(l);
      this.replace(end, end, '\n' + ind);
    } else {
      const st = this.starts[l];
      this.replace(st, st, ind + '\n', st + ind.length);
    }
  }

  copyLine(cut) {
    const l = this.lineOf(this.caret());
    const text = this.lines[l] + '\n';
    if (navigator.clipboard) navigator.clipboard.writeText(text).catch(() => {});
    if (cut) this.deleteLines();
  }

  /* Edit menu commands. */
  command(name) {
    const ta = this.ta;
    const s = ta.selectionStart, e = ta.selectionEnd;
    switch (name) {
      case 'undo': this.undo(); break;
      case 'redo': this.redo(); break;
      case 'cut':
      case 'copy':
        if (s === e) this.copyLine(name === 'cut');
        else {
          if (navigator.clipboard) navigator.clipboard.writeText(this.value.slice(s, e)).catch(() => {});
          if (name === 'cut') this.replace(s, e, '');
        }
        break;
      case 'paste':
        if (navigator.clipboard && navigator.clipboard.readText) {
          navigator.clipboard.readText().then((t) => this.replace(ta.selectionStart, ta.selectionEnd, t.replace(/\r\n?/g, '\n'))).catch(() => {});
        }
        break;
      case 'selectAll': ta.focus(); ta.setSelectionRange(0, this.value.length); this.onSelect(); break;
      case 'comment': this.toggleComment(); break;
      case 'indent': this.indentLines(false); break;
      case 'outdent': this.indentLines(true); break;
      case 'moveUp': this.moveLines(-1); break;
      case 'moveDown': this.moveLines(1); break;
      case 'duplicate': this.duplicateLines(true); break;
      case 'deleteLine': this.deleteLines(); break;
      case 'find': this.openFind(false); return;
      case 'replace': this.openFind(true); return;
      case 'suggest': this.focus(); this.openCompletion(true); return;
      default: break;
    }
    this.focus();
  }

  /* ---------------------------------------------------------------- find */

  openFind(replace) {
    this.fwOpen = true;
    this.fw.classList.add('open');
    this.showReplace(replace);
    const sel = this.selectedText();
    if (sel && !sel.includes('\n') && sel.length < 300) this.fwFind.value = sel;
    (replace && this.fwFind.value ? this.fwRep : this.fwFind).focus();
    this.fwFind.select();
    this.computeMatches(true);
  }

  showReplace(on) {
    this.fw.querySelector('.fw-replace-row').style.display = on ? 'flex' : 'none';
    this.fw.querySelector('.fw-toggle-replace').textContent = on ? '⌄' : '›';
    if (on) this.fwRep.focus();
  }

  closeFind() {
    this.fwOpen = false;
    this.fw.classList.remove('open');
    this.schedule();
    this.focus();
  }

  findRegex(global) {
    const f = this.find;
    let src = f.regex ? f.query : escRe(f.query);
    if (f.word) src = '(?<![\\w$])(?:' + src + ')(?![\\w$])';
    return new RegExp(src, (global ? 'g' : '') + (f.caseSens ? '' : 'i') + 'm');
  }

  computeMatches(reveal) {
    const f = this.find;
    f.query = this.fwFind.value;
    f.error = false;
    f.matches = [];
    if (f.query) {
      let re = null;
      try { re = this.findRegex(true); } catch (err) { f.error = true; }
      if (re) {
        const v = this.value;
        let m;
        while ((m = re.exec(v)) && f.matches.length < 20000) {
          if (!m[0].length) { re.lastIndex++; continue; }
          f.matches.push([m.index, m.index + m[0].length]);
        }
      }
    }
    if (f.matches.length) {
      const pos = this.ta.selectionStart;
      const k = f.matches.findIndex((m) => m[0] >= pos);
      f.current = k < 0 ? 0 : k;
      if (reveal) this.revealOffset(f.matches[f.current][0], true);
    } else f.current = -1;
    this.updateFindCount();
    this.schedule();
  }

  updateFindCount(text) {
    const f = this.find;
    this.fwCount.textContent = text || (f.error ? 'Expression invalide' : !f.query ? '' : f.matches.length ? `${f.current + 1} sur ${f.matches.length}` : 'Aucun résultat');
    this.fwFind.parentElement.style.borderColor = f.query && !f.matches.length ? 'var(--error)' : '';
  }

  findNext(dir) {
    const f = this.find;
    if (!this.fwOpen) { this.openFind(false); return; }
    const n = f.matches.length;
    if (!n) return;
    const ta = this.ta, s = ta.selectionStart, e = ta.selectionEnd;
    const cur = f.matches[f.current];
    let idx;
    if (cur && cur[0] === s && cur[1] === e) idx = (f.current + dir + n) % n;
    else if (dir > 0) { idx = f.matches.findIndex((m) => m[0] >= e); if (idx < 0) idx = 0; }
    else { idx = n - 1; for (let k = n - 1; k >= 0; k--) if (f.matches[k][1] <= s) { idx = k; break; } }
    f.current = idx;
    ta.setSelectionRange(f.matches[idx][0], f.matches[idx][1]);
    this.revealOffset(f.matches[idx][0], true);
    this.updateFindCount();
    this.schedule();
  }

  replacement(matchText) {
    const r = this.fwRep.value;
    if (!this.find.regex) return r;
    try { return matchText.replace(this.findRegex(false), r); } catch (err) { return r; }
  }

  replaceOne() {
    const f = this.find;
    if (!f.matches.length) return;
    const ta = this.ta, s = ta.selectionStart, e = ta.selectionEnd;
    const cur = f.matches[f.current];
    if (!cur || cur[0] !== s || cur[1] !== e) { this.findNext(1); return; }
    const rep = this.replacement(this.value.slice(s, e));
    this.replace(s, e, rep, s + rep.length);
    this.computeMatches(false);
    if (f.matches.length) this.findNext(1);
    this.fwRep.focus();
  }

  replaceAll() {
    const f = this.find;
    const n = f.matches.length;
    if (!n) return;
    const v = this.value;
    const a = f.matches[0][0], b = f.matches[n - 1][1];
    let out = '', pos = a;
    for (const [s, e] of f.matches) { out += v.slice(pos, s) + this.replacement(v.slice(s, e)); pos = e; }
    out += v.slice(pos, b);
    this.replace(a, b, out, a);
    this.computeMatches(false);
    this.updateFindCount(`${n} remplacé${n > 1 ? 's' : ''}`);
    this.fwRep.focus();
  }

  /* ---------------------------------------------------------------- completion */

  docWords() {
    if (this.wordsVersion !== this.version) {
      const set = new Set();
      const re = /[A-Za-z_][\w]{2,}/g;
      const v = this.value.length > 2000000 ? this.value.slice(0, 2000000) : this.value;
      let m;
      while ((m = re.exec(v)) && set.size < 4000) set.add(m[0]);
      this.words = [...set];
      this.wordsVersion = this.version;
    }
    return this.words;
  }

  completionContext(manual) {
    const ta = this.ta, pos = ta.selectionStart;
    if (pos !== ta.selectionEnd) return null;
    const line = this.lineOf(pos), t = this.lines[line], col = pos - this.starts[line];
    const lang = this.inEmbedded(line) || this.lang;
    const wordRe = lang === 'css' || lang === 'html' ? /[\w-]/ : /[\w$]/;
    let s = col;
    while (s > 0 && wordRe.test(t[s - 1])) s--;
    const prefix = t.slice(s, col), before = t.slice(0, s), after = t.slice(col);
    const tok = this.tokAt(line, Math.max(0, col - 1));
    if (tok && col > 0 && (tok[2] === 'comment' || (tok[2] === 'str' && lang !== 'html' && col < tok[1]))) return null;
    const words = (kind) => this.docWords().filter((w) => w !== prefix).map((w) => ({ label: w, kind }));
    let items = [];
    if (lang === 'nexul' || lang === 'js') {
      if (before.endsWith('.')) {
        const recv = (/([A-Za-z_$][\w$]*)\s*\.$/.exec(before) || [])[1];
        if (lang === 'nexul') {
          let names;
          if (recv === 'std') items = NX_MEMBERS.std.map((n) => ({ label: n, kind: 'type', detail: 'module' }));
          else {
            names = recv && NX_MEMBERS[recv] ? NX_MEMBERS[recv]
              : [...new Set([].concat(NX_MEMBERS.String, NX_MEMBERS.Vec, NX_MEMBERS.Option, NX_MEMBERS.Result, NX_MEMBERS.num))];
            items = names.map((n) => (/^[A-Z_0-9]+$/.test(n) ? { label: n, kind: 'const' } : { label: n, kind: 'fn', body: n + '($0)' }));
          }
        } else items = JS_MEMBERS.map((n) => ({ label: n, kind: 'fn' }));
        const seen = new Set(items.map((x) => x.label));
        const re = /\.([A-Za-z_][\w]*)/g;
        let m;
        while ((m = re.exec(this.value)) && seen.size < 600) {
          if (!seen.has(m[1]) && m[1] !== prefix) { seen.add(m[1]); items.push({ label: m[1], kind: 'var' }); }
        }
      } else {
        if (!manual && !prefix) return null;
        if (isDigit(prefix[0])) return null;
        if (lang === 'nexul') {
          items = NX_SNIPPETS.map((x) => ({ ...x, kind: 'snip' }))
            .concat(NX_WORDS.map((x) => (x.kind === 'fn' ? { ...x, body: x.label + '($0)' } : x)), words('var'));
        } else {
          items = JS_SNIPPETS.map((x) => ({ ...x, kind: 'snip' })).concat(JS_WORDS.map((w) => ({ label: w, kind: 'kw' })), words('var'));
        }
      }
    } else if (lang === 'css') {
      const st = this.stateAt(line);
      const opens = (before.match(/\{/g) || []).length - (before.match(/\}/g) || []).length;
      const depth = (this.inEmbedded(line) ? 1 : st[1]) + opens;
      const seg = before.slice(Math.max(before.lastIndexOf('{'), before.lastIndexOf(';')) + 1);
      if (depth <= 0 && !this.inEmbedded(line)) {
        if (!manual && !prefix) return null;
        items = HTML_TAGS.map((n) => ({ label: n, kind: 'type' }));
      } else if (seg.includes(':')) {
        if (!manual && !prefix) return null;
        items = CSS_VALUES.map((v) => (v.endsWith('()') ? { label: v.slice(0, -2), kind: 'fn', body: v.slice(0, -1) + '$0)' } : { label: v, kind: 'const' }));
      } else {
        if (!manual && !prefix) return null;
        const hasColon = /^\s*:/.test(after);
        items = CSS_PROPS.map((p) => ({ label: p, kind: 'kw', body: hasColon ? undefined : p + ': $0;' }));
      }
    } else if (lang === 'html') {
      const lt = before.lastIndexOf('<'), gt = before.lastIndexOf('>');
      if (lt > gt) {
        const inTag = before.slice(lt + 1);
        if (/^\/?[\w-]*$/.test(inTag)) {
          const closing = inTag.startsWith('/');
          const open = closing ? this.unclosedTag(this.starts[line] + lt) : null;
          items = HTML_TAGS.map((n) => ({ label: n, kind: 'type', boost: n === open ? 20 : 0, body: closing && !after.startsWith('>') ? n + '>' : undefined }));
        } else {
          if (((inTag.match(/"/g) || []).length % 2) || ((inTag.match(/'/g) || []).length % 2)) return null;
          if (!manual && !prefix) return null;
          const eq = after.startsWith('=');
          items = HTML_ATTRS.map((a) => ({ label: a, kind: 'kw', body: eq ? undefined : a + '="$0"' }));
        }
      } else {
        if (!prefix && !manual) return null;
        if (!/^[a-zA-Z]/.test(prefix) && prefix) return null;
        items = HTML_TAGS.map((n) => ({ label: n, kind: 'snip', detail: '<' + n + '>', html: true }));
      }
    } else {
      if (!manual && prefix.length < 3) return null;
      items = words('var');
    }
    return { from: pos - prefix.length, line, prefix, items };
  }

  openCompletion(manual) {
    const ctx = this.completionContext(manual);
    if (!ctx) return this.closeCompletion();
    const p = ctx.prefix.toLowerCase();
    const seen = new Set();
    let list = [];
    for (const it of ctx.items) {
      const key = it.label + (it.kind === 'snip' ? '#' : '');
      if (seen.has(key)) continue;
      seen.add(key);
      if (!p) { list.push({ it, score: it.boost || 0, hl: [] }); continue; }
      if (it.label === ctx.prefix && it.kind !== 'snip') continue;
      const m = fuzzy(it.label, p);
      if (m) list.push({ it, score: m.score + (it.boost || 0) + (it.kind === 'var' ? -1 : 0) + (it.label.startsWith(ctx.prefix) ? 1 : 0), hl: m.hl });
    }
    // Without a prefix the lists keep their order (the most useful names first).
    if (p) list.sort((a, b) => b.score - a.score || a.it.label.length - b.it.label.length || a.it.label.localeCompare(b.it.label));
    else list.sort((a, b) => b.score - a.score);
    list = list.slice(0, 80);
    if (!list.length) return this.closeCompletion();
    this.cmp = { from: ctx.from, line: ctx.line, items: list, index: 0 };
    this.renderCompletion();
  }

  renderCompletion() {
    const c = this.cmp;
    if (!c) return;
    if (!this.cmpEl) {
      const el = document.createElement('div');
      el.className = 'completion';
      el.addEventListener('mousedown', (e) => {
        e.preventDefault();
        const item = e.target.closest('.cmp-item');
        if (item && this.cmp) { this.cmp.index = +item.dataset.i; this.acceptCompletion(); }
      });
      document.body.appendChild(el);
      this.cmpEl = el;
    }
    this.cmpEl.innerHTML = c.items.map((x, i) => {
      const [kc, kl] = KIND_BADGE[x.it.kind] || KIND_BADGE.var;
      return `<div class="cmp-item${i === c.index ? ' active' : ''}" data-i="${i}"><span class="cmp-kind ${kc}">${kl}</span>` +
        `<span class="cmp-label">${highlightLabel(x.it.label, x.hl)}</span><span class="cmp-detail">${escHtml(x.it.detail || '')}</span></div>`;
    }).join('');
    this.positionCompletion();
  }

  positionCompletion() {
    const c = this.cmp, el = this.cmpEl;
    const { cw, lh } = Editor.metrics;
    const r = this.view.getBoundingClientRect();
    const t = this.lines[c.line] || '';
    const x = r.left + 10 + this.vcol(t, c.from - this.starts[c.line]) * cw - this.ta.scrollLeft - 34;
    let y = r.top + (c.line + 1) * lh - this.ta.scrollTop + 2;
    const h = el.offsetHeight;
    if (y + h > innerHeight - 8) y = y - lh - h - 4;
    el.style.left = clamp(x, 4, innerWidth - el.offsetWidth - 4) + 'px';
    el.style.top = y + 'px';
  }

  moveCmp(d) {
    const c = this.cmp, n = c.items.length;
    const old = c.index;
    c.index = d === 8 || d === -8 ? clamp(c.index + d, 0, n - 1) : (c.index + d + n) % n;
    const kids = this.cmpEl.children;
    kids[old].classList.remove('active');
    kids[c.index].classList.add('active');
    kids[c.index].scrollIntoView({ block: 'nearest' });
  }

  acceptCompletion() {
    const c = this.cmp;
    if (!c) return;
    const x = c.items[c.index];
    this.closeCompletion();
    if (!x) return;
    const it = x.it;
    const to = this.ta.selectionStart;
    const next = this.value[to] || '';
    if (it.html) {
      const body = VOID_TAGS.has(it.label) ? `<${it.label}>` : `<${it.label}>$0</${it.label}>`;
      this.insertSnippet(body, c.from, to);
    } else if (it.body && !(it.kind === 'fn' && next === '(')) {
      this.insertSnippet(it.body, c.from, to);
    } else {
      this.accepting = true;
      this.replace(c.from, to, it.label);
      this.accepting = false;
    }
  }

  closeCompletion() {
    this.cmp = null;
    if (this.cmpEl) { this.cmpEl.remove(); this.cmpEl = null; }
  }

  /* ---------------------------------------------------------------- hover */

  onHover(e) {
    if (e.buttons) return;
    clearTimeout(this.hoverTimer);
    const r = this.view.getBoundingClientRect();
    const { cw, lh } = Editor.metrics;
    const line = Math.floor((e.clientY - r.top + this.ta.scrollTop) / lh);
    if (line < 0 || line >= this.lines.length) return this.hideHoverSoon();
    const vx = (e.clientX - r.left + this.ta.scrollLeft - 10) / cw;
    const t = this.lines[line];
    let c = 0, k = 0;
    while (k < t.length) {
      const w = t[k] === '\t' ? TAB_W - (c % TAB_W) : 1;
      if (c + w > vx) break;
      c += w;
      k++;
    }
    const hit = vx < 0 ? null : this.diagAt(line, k);
    if (!hit) return this.hideHoverSoon();
    if (this.hoverDiag === hit.d && this.tipEl) { clearTimeout(this.hideTimer); return; }
    this.hoverTimer = setTimeout(() => this.showHover(hit.d, e.clientX, r.top + (line + 1) * lh - this.ta.scrollTop), 300);
  }

  showHover(d, x, y) {
    this.hideHover();
    const tip = document.createElement('div');
    tip.className = 'hover-tip';
    const lvl = d.level === 'error' ? 'err' : 'warn';
    let html = `<div class="tip-title">${d.code ? `<span class="tip-code ${lvl}">${escHtml(d.code)}</span>` : ''}<span>${fmtMsg(d.message)}</span></div>`;
    for (const lb of d.labels) {
      if (!lb.message) continue;
      html += `<div class="tip-note">${lb.primary ? '→ ' : `ligne ${lb.line} : `}${fmtMsg(lb.message)}</div>`;
    }
    for (const n of d.notes) html += `<div class="tip-note">${fmtMsg(n)}</div>`;
    if (d.code && this.hooks.explain) html += `<div style="margin-top:8px"><a data-explain="1">Expliquer ${escHtml(d.code)} →</a></div>`;
    tip.innerHTML = html;
    document.body.appendChild(tip);
    const w = tip.offsetWidth, h = tip.offsetHeight;
    let top = y + 4;
    if (top + h > innerHeight - 8) top = y - Editor.metrics.lh - h - 6;
    tip.style.left = clamp(x - 24, 8, innerWidth - w - 8) + 'px';
    tip.style.top = Math.max(8, top) + 'px';
    tip.addEventListener('mouseenter', () => clearTimeout(this.hideTimer));
    tip.addEventListener('mouseleave', () => this.hideHoverSoon());
    const a = tip.querySelector('a[data-explain]');
    if (a) a.addEventListener('click', () => { this.hideHover(); this.hooks.explain(d.code); });
    this.tipEl = tip;
    this.hoverDiag = d;
  }

  hideHoverSoon() {
    clearTimeout(this.hoverTimer);
    clearTimeout(this.hideTimer);
    if (this.tipEl) this.hideTimer = setTimeout(() => this.hideHover(), 250);
  }

  hideHover() {
    clearTimeout(this.hideTimer);
    if (this.tipEl) { this.tipEl.remove(); this.tipEl = null; }
    this.hoverDiag = null;
  }
}
