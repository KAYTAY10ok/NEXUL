'use strict';
/* NEXUL Studio — language services of the compiler (`nxc ide`), outline, tests,
 * debugger (lldb-dap) and packages (nxpm). Loaded after app.js. */

/* ==================================================================== icons */

Object.assign(ICON_PATHS, {
  outline: '<path d="M4 6h16M4 12h10M4 18h13"/><circle cx="20" cy="12" r="1.2"/>',
  bug: '<path d="M8 8a4 4 0 0 1 8 0v5a4 4 0 0 1-8 0z"/><path d="M12 13v8M3 13h5M16 13h5M5 7l3 2M19 7l-3 2M5 20l3-3M19 20l-3-3M9 4l1.5 2M15 4l-1.5 2"/>',
  box: '<path d="M21 8 12 3 3 8v8l9 5 9-5z"/><path d="M3 8l9 5 9-5M12 13v8"/>',
  puzzle: '<path d="M10 3h4v3a2 2 0 1 0 4 0V3h3v7h-3a2 2 0 1 0 0 4h3v7h-7v-3a2 2 0 1 0-4 0v3H3v-7h3a2 2 0 1 0 0-4H3V3z"/>',
  stepOver: '<path d="M4 12a8 8 0 0 1 15-4"/><path d="M19 3v5h-5"/><circle cx="12" cy="18" r="2"/>',
  stepIn: '<path d="M12 3v11"/><path d="m7 9 5 5 5-5"/><circle cx="12" cy="20" r="2"/>',
  stepOut: '<path d="M12 15V4"/><path d="m7 9 5-5 5 5"/><circle cx="12" cy="20" r="2"/>',
  cont: '<path d="M6 4v16"/><path d="m10 4 10 8-10 8z"/>',
});

/* ==================================================================== compiler requests */

const isNx = (doc) => !!(doc && doc.ed && extOf(doc.path) === 'nx' && !doc.virtual);
const docOf = (ed) => S.docs.find((d) => d.ed === ed);

function cpLen(s) { return [...s].length; }

/* `nxc ide <req>` on the text of the editor; positions are 1-based, in code points. */
async function ideRequest(req, doc, pos, extra = {}) {
  const params = { req, path: doc.path, line: pos ? pos.line : 1, col: pos ? pos.col : 1, ...extra };
  return api.req('POST', 'ide?' + qs(params), doc.ed.value);
}

/* Offset (UTF-16) of a 1-based position in code points. */
function offsetIn(text, line, col) {
  let off = 0;
  for (let l = 1; l < line; l++) {
    const nl = text.indexOf('\n', off);
    if (nl < 0) return text.length;
    off = nl + 1;
  }
  const end = text.indexOf('\n', off);
  const lineText = text.slice(off, end < 0 ? text.length : end);
  return off + cpIndex(lineText, col - 1);
}

/* ==================================================================== editor hooks */

editorHooks.complete = async (ed) => {
  const doc = docOf(ed);
  if (!isNx(doc) || !S.nxc) return [];
  const list = await ideRequest('completion', doc, ed.caretPosition());
  if (!Array.isArray(list)) return [];
  const KIND = { function: 'fn', method: 'fn', field: 'var', variable: 'var', keyword: 'kw', struct: 'type', enum: 'type', trait: 'type', type: 'type', module: 'type', constant: 'const', static: 'const', variant: 'const' };
  return list.filter((x) => x.kind !== 'keyword').map((x) => ({
    label: x.label,
    kind: KIND[x.kind] || 'var',
    detail: x.detail || '',
    doc: x.doc,
    boost: x.kind === 'variable' || x.kind === 'field' ? 3 : x.kind === 'method' ? 2 : 0,
    body: x.kind === 'function' || x.kind === 'method' ? x.label + (x.detail && /\(\)/.test(x.detail.replace(/\(self\)/, '()')) ? '()' : '($0)') : undefined,
  }));
};

editorHooks.hoverInfo = async (ed, line, col) => {
  const doc = docOf(ed);
  if (!isNx(doc) || !S.nxc) return null;
  return ideRequest('hover', doc, { line, col });
};

editorHooks.definition = async (ed) => {
  const doc = docOf(ed);
  if (!isNx(doc) || !S.nxc) return;
  let r;
  try { r = await ideRequest('definition', doc, ed.caretPosition()); } catch (e) { toast(e.message, 'error'); return; }
  if (!r) { toast('Aucune définition trouvée ici.', 'info', 2000); return; }
  await openLocation(doc, r);
};

/* Opens the place designated by a position of `nxc ide` (relative to `doc`). */
async function openLocation(doc, r) {
  if (r.std) {
    const module = r.file.replace(/^<|>$/g, '');
    const v = await openVirtual('std:' + module, async () => api.get('std-source?' + qs({ module })));
    if (v) v.ed.goTo(r.line - 1, cpIndex(v.ed.lines[r.line - 1] || '', r.col - 1));
    return;
  }
  const path = r.file ? normRel(joinRel(dirName(doc.path), r.file)) : doc.path;
  await openFile(path, { line: r.line - 1, charCol: r.col - 1 });
}

/* A read-only document that is not a file of the folder (source of the standard library). */
async function openVirtual(path, load) {
  let doc = S.docs.find((d) => d.path === path);
  if (!doc) {
    let text;
    try { text = await load(); } catch (e) { toast(e.message, 'error'); return null; }
    const ed = new Editor({ path, lang: 'nexul', text }, editorHooks);
    ed.ta.readOnly = true;
    doc = { path, name: path.replace(/^std:/, '') + ' (lecture seule)', ed, el: ed.el, virtual: true };
    const i = S.active ? S.docs.indexOf(S.active) + 1 : S.docs.length;
    S.docs.splice(i, 0, doc);
    $('#editor-container').append(doc.el);
  }
  activate(doc);
  return doc;
}

editorHooks.references = async (ed) => {
  const doc = docOf(ed);
  if (!isNx(doc) || !S.nxc) return;
  let refs;
  try { refs = await ideRequest('references', doc, ed.caretPosition()); } catch (e) { toast(e.message, 'error'); return; }
  if (!refs || !refs.length) { toast('Aucune référence trouvée.', 'info', 2000); return; }
  const items = refs.map((r) => ({ path: r.file ? normRel(joinRel(dirName(doc.path), r.file)) : doc.path, r }));
  renderReferences(items);
  showPanel('references');
};

editorHooks.rename = async (ed) => {
  const doc = docOf(ed);
  if (!isNx(doc) || !S.nxc) return;
  const pos = ed.caretPosition();
  const t = ed.lines[pos.line - 1];
  const k = cpIndex(t, pos.col - 1);
  let s = k, e = k;
  while (s > 0 && /\w/.test(t[s - 1])) s--;
  while (e < t.length && /\w/.test(t[e])) e++;
  const old = t.slice(s, e);
  if (!old) return;
  const name = await promptBox(`Renommer « ${old} »`, { label: 'Nouveau nom', value: old, select: true, ok: 'Renommer' });
  if (!name || name === old) return;
  await saveAll();
  let r;
  try { r = await ideRequest('rename', doc, pos, { name }); } catch (err) { toast(err.message, 'error'); return; }
  if (!r || r.error) { toast(r ? r.error : 'Rien à renommer ici.', 'error'); return; }
  // Edits by file, applied from the end.
  const byFile = new Map();
  for (const x of r) {
    const p = x.file ? normRel(joinRel(dirName(doc.path), x.file)) : doc.path;
    if (!byFile.has(p)) byFile.set(p, []);
    byFile.get(p).push(x);
  }
  let count = 0;
  for (const [p, list] of byFile) {
    const open = S.docs.find((d) => d.path === p && d.ed);
    let text = open ? open.ed.value : await textOf(p);
    list.sort((a, b) => b.line - a.line || b.col - a.col);
    for (const x of list) {
      const a = offsetIn(text, x.line, x.col), b = offsetIn(text, x.end_line, x.end_col);
      text = text.slice(0, a) + name + text.slice(b);
      count++;
    }
    if (open) {
      open.ed.replace(0, open.ed.value.length, text, Math.min(open.ed.ta.selectionStart, text.length));
      await saveDoc(open, true);
    } else {
      await api.post('file?' + qs({ path: p }), text);
    }
  }
  toast(`« ${old} » renommé en « ${name} » (${count} occurrence${count > 1 ? 's' : ''}).`, 'success', 3000);
  scheduleCheck(doc.path, 50);
};

editorHooks.format = (ed) => formatDoc(docOf(ed), true);

async function formatDoc(doc, manual) {
  if (!isNx(doc) || !S.nxc) return;
  let text;
  try { text = await ideRequest('format', doc, null); } catch (e) { if (manual) toast(e.message, 'error'); return; }
  if (typeof text !== 'string' || !text || text === doc.ed.value) { if (manual) toast('Le fichier est déjà bien formaté.', 'info', 1800); return; }
  const ed = doc.ed;
  const pos = ed.caretPosition();
  ed.replace(0, ed.value.length, text, 0);
  const l = Math.min(pos.line - 1, ed.lines.length - 1);
  ed.goTo(l, Math.min(cpIndex(ed.lines[l], pos.col - 1), ed.lines[l].length));
}

/* Format on save (setting). */
const _saveDoc = saveDoc;
saveDoc = async function (doc, manual) {
  if (settings.formatOnSave && manual && isNx(doc)) await formatDoc(doc, false);
  return _saveDoc(doc, manual);
};

/* ==================================================================== references panel */

function renderReferences(items) {
  let view = $('.panel-view[data-view="references"]');
  if (!view) return;
  view.innerHTML = '';
  const box = h('div', { class: 'problems' });
  const byFile = new Map();
  for (const it of items) { if (!byFile.has(it.path)) byFile.set(it.path, []); byFile.get(it.path).push(it); }
  for (const [path, list] of byFile) {
    box.append(h('div', { class: 'prob-file' }, fileIconEl(baseName(path)), h('span', { text: baseName(path) }), h('span', { class: 'count', text: list.length })));
    for (const it of list) {
      const doc = S.docs.find((d) => d.path === path && d.ed);
      const line = doc ? (doc.ed.lines[it.r.line - 1] || '').trim() : '';
      box.append(h('div', { class: 'prob-item', onclick: () => openFile(path, { line: it.r.line - 1, charCol: it.r.col - 1, len: Math.max(0, it.r.end_col - it.r.col) }) },
        iconEl('code'), h('span', { class: 'msg', text: line || `ligne ${it.r.line}` }), h('span', { class: 'where', text: `ligne ${it.r.line}, col. ${it.r.col}` })));
    }
  }
  view.append(box);
  const badge = $('.panel-tab[data-tab="references"] .count');
  if (badge) badge.textContent = items.length;
}

/* ==================================================================== sidebar views */

const IDE_VIEWS = [
  ['outline', 'outline', 'Structure du fichier'],
  ['tests', 'flask', 'Tests (F6)'],
  ['debug', 'bug', 'Débogage (F9)'],
  ['packages', 'box', 'Paquets (nxpm)'],
  ['plugins', 'puzzle', 'Extensions'],
];

function installViews() {
  const ab = $('#activitybar');
  const spacer = $('.act-spacer', ab);
  for (const [view, ic, title] of IDE_VIEWS) {
    ab.insertBefore(h('button', { class: 'act-btn', dataset: { view }, title, html: icon(ic), onclick: () => showView(view, true) }), spacer);
    $('#sidebar').append(h('div', { class: 'sb-view', dataset: { view } }));
  }
  // Panel: references and debug console.
  const head = $('.panel-head');
  const spacerP = $('.panel-spacer', head);
  head.insertBefore(h('button', { class: 'panel-tab', dataset: { tab: 'references' }, html: 'Références <span class="count">0</span>', onclick: () => showPanel('references') }), spacerP);
  head.insertBefore(h('button', { class: 'panel-tab', dataset: { tab: 'debugconsole' }, html: 'Console de débogage', onclick: () => showPanel('debugconsole') }), spacerP);
  const body = $('.panel-body');
  body.append(h('div', { class: 'panel-view', dataset: { view: 'references' } }));
  const dc = h('div', { class: 'panel-view', dataset: { view: 'debugconsole' } },
    h('div', { class: 'term-out dbg-out' }),
    h('div', { class: 'term-input' }, h('span', { text: '›' }), h('input', { placeholder: 'Expression à évaluer pendant une pause (ex. : total, liste.len)', spellcheck: 'false', id: 'dbg-eval' })));
  body.append(dc);
  $('#dbg-eval').addEventListener('keydown', (e) => { if (e.key === 'Enter') { const v = e.target.value.trim(); e.target.value = ''; if (v) DBG.evaluate(v); } });
}

const _showView = showView;
showView = function (view, fromActivity) {
  _showView(view, fromActivity);
  for (const b of $$('.act-btn[data-view]')) b.classList.toggle('active', !$('#sidebar').classList.contains('hidden') && b.dataset.view === S.sidebarView);
  if (view === 'outline') renderOutline();
  if (view === 'tests') renderTests();
  if (view === 'debug') renderDebug();
  if (view === 'packages') renderPackages();
  if (view === 'plugins' && window.PLUGINS) PLUGINS.render();
};

const viewEl = (v) => $(`.sb-view[data-view="${v}"]`);
const viewHead = (title, ...buttons) => h('div', { class: 'sb-head' }, h('span', { text: title }), h('div', { class: 'sb-actions' }, ...buttons));
const iconBtn = (ic, title, onclick) => h('button', { class: 'icon-btn', title, html: icon(ic), onclick });

/* ---------------------------------------------------------------- outline */

const OUTLINE_KIND = { function: ['ƒ', 'k-fn'], test: ['✓', 'k-fn'], method: ['ƒ', 'k-fn'], struct: ['S', 'k-type'], enum: ['E', 'k-type'], trait: ['T', 'k-type'], field: ['v', 'k-var'], variant: ['C', 'k-type'], constant: ['C', 'k-type'], static: ['S', 'k-var'] };

async function renderOutline() {
  const el = viewEl('outline');
  if (!el || S.sidebarView !== 'outline') return;
  const doc = S.active;
  el.innerHTML = '';
  el.append(viewHead('STRUCTURE', iconBtn('refresh', 'Actualiser', () => renderOutline())));
  const box = h('div', { class: 'sb-list' });
  el.append(box);
  if (!isNx(doc)) { box.append(h('div', { class: 'sb-empty', text: 'Ouvrez un fichier NEXUL (.nx) pour voir ses fonctions, types et traits.' })); return; }
  let syms;
  try { syms = await ideRequest('symbols', doc, null); } catch (e) { box.append(h('div', { class: 'sb-empty', text: e.message })); return; }
  if (S.active !== doc) return;
  if (!syms || !syms.length) { box.append(h('div', { class: 'sb-empty', text: 'Aucune déclaration.' })); return; }
  const row = (s, depth) => {
    const [k, cls] = OUTLINE_KIND[s.kind] || ['•', 'k-var'];
    box.append(h('div', {
      class: 'sb-row', style: { paddingLeft: 10 + depth * 16 + 'px' }, title: s.detail || s.name,
      onclick: () => openFile(doc.path, { line: s.line - 1, charCol: s.col - 1, len: s.name.length }),
    }, h('span', { class: 'cmp-kind ' + cls, text: k }), h('span', { class: 'sb-name', text: s.name }), h('span', { class: 'sb-detail', text: s.kind === 'field' ? s.detail : '' })));
    for (const c of s.children || []) row(c, depth + 1);
  };
  for (const s of syms) row(s, 0);
}

/* ---------------------------------------------------------------- tests */

const TESTS = { results: new Map() };

async function renderTests() {
  const el = viewEl('tests');
  if (!el || S.sidebarView !== 'tests') return;
  const doc = S.active && extOf(S.active.path) === 'nx' && !S.active.virtual ? S.active : null;
  el.innerHTML = '';
  el.append(viewHead('TESTS', iconBtn('play', 'Lancer tous les tests du fichier (F6)', () => runTests(doc, '')), iconBtn('refresh', 'Actualiser', () => renderTests())));
  const box = h('div', { class: 'sb-list' });
  el.append(box);
  if (!doc) { box.append(h('div', { class: 'sb-empty', html: 'Ouvrez un fichier NEXUL contenant des fonctions <code>@test</code>.' })); return; }
  let tests;
  try { tests = await ideRequest('tests', doc, null); } catch (e) { box.append(h('div', { class: 'sb-empty', text: e.message })); return; }
  if (!tests || !tests.length) {
    box.append(h('div', { class: 'sb-empty', html: 'Aucun test dans ce fichier.<br>Écrivez <code>@test</code> devant une fonction sans paramètres, puis utilisez <code>assert(...)</code>.' }));
    return;
  }
  for (const t of tests) {
    const st = TESTS.results.get(doc.path + '#' + t.name);
    const mark = st === 'ok' ? h('span', { class: 'test-ok', text: '✓' }) : st === 'fail' ? h('span', { class: 'test-fail', text: '✗' }) : st === 'run' ? h('span', { class: 'test-run', text: '…' }) : h('span', { class: 'test-none', text: '○' });
    box.append(h('div', { class: 'sb-row', onclick: () => openFile(doc.path, { line: t.line - 1 }) },
      mark, h('span', { class: 'sb-name', text: t.name }),
      h('button', { class: 'icon-btn sb-inline', title: 'Lancer ce test', html: icon('play'), onclick: (e) => { e.stopPropagation(); runTests(doc, t.name); } })));
  }
}

async function runTests(doc, filter) {
  if (!doc || !S.nxc) { toast('Ouvrez un fichier NEXUL contenant des tests.', 'error'); return; }
  if (S.run) await stopRun(true);
  await saveAll();
  showPanel('terminal');
  termClear();
  let tests = [];
  try { tests = (await ideRequest('tests', doc, null)) || []; } catch (e) { /* the run reports it */ }
  for (const t of tests) if (!filter || t.name.includes(filter)) TESTS.results.set(doc.path + '#' + t.name, 'run');
  renderTests();
  let r;
  try { r = await api.post('run?' + qs({ path: doc.path, kind: 'test' }), filter); } catch (e) { termWrite(`\x1b[31m${e.message}\x1b[0m\n`); return; }
  S.run = { id: r.id, from: 0, kind: 'test', path: doc.path, started: performance.now(), testDoc: doc, testOut: '' };
  updateRunUi();
  pollRun(S.run);
}

/* Test results are read from the output of `nxc test`. */
const _termWrite = termWrite;
termWrite = function (text, runInfo) {
  if (runInfo && runInfo.testDoc) {
    runInfo.testOut += text;
    const re = /^test (?:[\w.]+\.)?(\w+) \.\.\. (ok|FAILED)/gm;
    let m;
    let changed = false;
    while ((m = re.exec(runInfo.testOut))) { TESTS.results.set(runInfo.testDoc.path + '#' + m[1], m[2] === 'ok' ? 'ok' : 'fail'); changed = true; }
    if (changed) renderTests();
  }
  return _termWrite(text, runInfo);
};

CMD.test.run = () => {
  const doc = S.active && extOf(S.active.path) === 'nx' ? S.active : null;
  if (doc) return runTests(doc, '');
  return runProgram('test');
};

/* ==================================================================== debugger */

const DBG = {
  on: false, seq: 1, pending: new Map(), from: 0, timer: 0,
  threadId: null, frames: [], frameIndex: 0, scopes: [], vars: new Map(), expanded: new Set(),
  root: '', sep: '\\', status: '',

  async start() {
    if (this.on) { this.request('continue', { threadId: this.threadId }); return; }
    const doc = S.active && extOf(S.active.path) === 'nx' && !S.active.virtual ? S.active : null;
    let path = doc ? doc.path : findMainNx();
    if (!path) { toast('Ouvrez le programme NEXUL à déboguer.', 'error'); return; }
    path = (await checkRoots(path))[0];
    if (S.run) await stopRun(true);
    await saveAll();
    this.reset();
    this.setStatus('Compilation…');
    showView('debug');
    showPanel('debugconsole');
    $('.dbg-out').innerHTML = '';
    let r;
    try { r = await api.post('debug/launch?' + qs({ path })); } catch (e) { this.setStatus(''); toast(e.message, 'error'); return; }
    if (r.error) { this.setStatus(''); showPanel('terminal'); termClear(); _termWrite(r.error, { path }); return; }
    this.on = true;
    this.root = r.root;
    this.sep = r.sep;
    this.setStatus('Démarrage…');
    this.poll();
    await this.request('initialize', { clientID: 'nexul-studio', clientName: 'NEXUL Studio', adapterID: 'lldb-dap', linesStartAt1: true, columnsStartAt1: true, pathFormat: 'path', locale: 'fr-FR' });
    const argv = (settings.args || '').split(/\s+/).filter(Boolean);
    this.request('launch', { program: r.program, cwd: r.cwd, args: argv, stopOnEntry: false });
    this.renderAll();
  },

  reset() {
    this.on = false; this.seq = 1; this.pending.clear(); this.from = 0; clearTimeout(this.timer);
    this.threadId = null; this.frames = []; this.frameIndex = 0; this.scopes = []; this.vars.clear(); this.expanded.clear();
    this.setDebugLine(null);
  },

  request(command, args) {
    const seq = this.seq++;
    const msg = JSON.stringify({ seq, type: 'request', command, arguments: args || {} });
    return new Promise((resolve) => {
      this.pending.set(seq, resolve);
      api.req('POST', 'debug/send', msg).catch((e) => { this.pending.delete(seq); resolve({ success: false, message: e.message }); });
    });
  },

  async poll() {
    if (!this.on) return;
    let r;
    try { r = await api.get('debug/poll?' + qs({ from: this.from })); } catch (e) { this.timer = setTimeout(() => this.poll(), 300); return; }
    this.from = r.next;
    for (const m of r.messages) this.handle(m);
    if (r.done && !r.messages.length) { this.finish(); return; }
    this.timer = setTimeout(() => this.poll(), 60);
  },

  handle(m) {
    if (m.type === 'response') {
      const res = this.pending.get(m.request_seq);
      if (res) { this.pending.delete(m.request_seq); res(m); }
      if (!m.success && m.message && !['evaluate', 'disconnect'].includes(m.command)) this.console(`${m.command} : ${m.message}\n`, 'err');
      return;
    }
    if (m.type !== 'event') return;
    const b = m.body || {};
    switch (m.event) {
      case 'initialized': this.sendBreakpoints().then(() => this.request('configurationDone')); break;
      case 'process': this.setStatus('En cours d\'exécution'); break;
      case 'output':
        if (b.category === 'stdout' || b.category === 'stderr') this.console(b.output, b.category === 'stderr' ? 'err' : '');
        else if (b.category !== 'telemetry' && !/^To get started|^For more information/.test(b.output || '')) this.console(b.output, 'dim');
        break;
      case 'stopped': this.onStopped(b); break;
      case 'continued': this.setStatus('En cours d\'exécution'); this.setDebugLine(null); this.frames = []; this.renderAll(); break;
      case 'exited': this.console(`\nProgramme terminé avec le code ${b.exitCode}.\n`, b.exitCode ? 'err' : 'ok'); break;
      case 'terminated': this.stop(true); break;
      default: break;
    }
  },

  async onStopped(b) {
    this.threadId = b.threadId || this.threadId;
    const why = { breakpoint: "point d'arrêt", step: 'pas à pas', exception: 'exception', pause: 'pause', entry: 'entrée' }[b.reason] || b.reason;
    this.setStatus(`En pause (${why})`);
    const st = await this.request('stackTrace', { threadId: this.threadId, levels: 50 });
    this.frames = st.success ? st.body.stackFrames : [];
    this.frameIndex = Math.max(0, this.frames.findIndex((f) => f.source && f.source.path && this.relPath(f.source.path)));
    await this.selectFrame(this.frameIndex);
  },

  relPath(abs) {
    if (!abs || !this.root) return null;
    const a = abs.replace(/\\/g, '/'), r = this.root.replace(/\\/g, '/').replace(/\/$/, '') + '/';
    return a.toLowerCase().startsWith(r.toLowerCase()) ? a.slice(r.length) : null;
  },

  async selectFrame(i) {
    this.frameIndex = i;
    const f = this.frames[i];
    this.vars.clear();
    this.scopes = [];
    if (f) {
      const rel = f.source && this.relPath(f.source.path);
      if (rel) {
        const doc = await openFile(rel, { line: f.line - 1 });
        this.setDebugLine(doc && doc.ed ? { doc, line: f.line - 1 } : null);
      } else this.setDebugLine(null);
      const sc = await this.request('scopes', { frameId: f.id });
      this.scopes = sc.success ? sc.body.scopes.filter((s) => s.presentationHint !== 'registers' && s.name !== 'Globals') : [];
      for (const s of this.scopes) await this.loadVars(s.variablesReference);
    }
    this.renderAll();
  },

  async loadVars(ref) {
    if (!ref || this.vars.has(ref)) return;
    const r = await this.request('variables', { variablesReference: ref });
    this.vars.set(ref, r.success ? r.body.variables : []);
  },

  setDebugLine(x) {
    for (const d of S.docs) if (d.ed && d.ed.debugLine !== null && d.ed.debugLine !== undefined) { d.ed.debugLine = null; d.ed.gkey = ''; d.ed.dkey = ''; d.ed.schedule(); }
    if (x) { x.doc.ed.debugLine = x.line; x.doc.ed.gkey = ''; x.doc.ed.schedule(); }
  },

  async sendBreakpoints() {
    for (const [path, lines] of BREAKPOINTS) {
      const abs = absPath(path);
      await this.request('setBreakpoints', { source: { path: abs, name: baseName(path) }, breakpoints: [...lines].sort((a, b) => a - b).map((l) => ({ line: l + 1 })) });
    }
  },

  cmd(name) {
    if (!this.on || !this.threadId) return;
    this.setStatus('En cours d\'exécution');
    this.setDebugLine(null);
    this.frames = [];
    this.renderAll();
    this.request(name, { threadId: this.threadId });
  },

  async evaluate(expr) {
    if (!this.on) { this.console('Lancez le débogage (F9) puis mettez le programme en pause.\n', 'dim'); return; }
    this.console(`› ${expr}\n`, 'dim');
    const f = this.frames[this.frameIndex];
    const r = await this.request('evaluate', { expression: expr, frameId: f ? f.id : undefined, context: 'repl' });
    this.console((r.success ? r.body.result : r.message || 'erreur') + '\n', r.success ? '' : 'err');
  },

  async stop(fromAdapter) {
    if (!this.on) return;
    if (!fromAdapter) {
      // The adapter may already be gone: do not wait more than a second for its answer.
      await Promise.race([this.request('disconnect', { terminateDebuggee: true }), new Promise((r) => setTimeout(r, 1000))]);
    }
    this.finish();
  },

  finish() {
    if (!this.on) return;
    api.post('debug/stop').catch(() => {});
    this.reset();
    this.setStatus('');
    this.renderAll();
  },

  console(text, cls) {
    const out = $('.dbg-out');
    if (!out) return;
    const atBottom = out.scrollTop + out.clientHeight >= out.scrollHeight - 6;
    out.append(h('span', { class: cls === 'err' ? 'ansi-31' : cls === 'dim' ? 't-dim' : cls === 'ok' ? 'ansi-32' : '', text: (text || '').replace(/\r\n/g, '\n') }));
    if (atBottom) out.scrollTop = out.scrollHeight;
  },

  setStatus(s) {
    this.status = s;
    const el = $('#dbg-status');
    if (el) el.textContent = s;
    updateStatus();
  },

  renderAll() { if (S.sidebarView === 'debug') renderDebug(); },
};

/* Debugger type names as NEXUL writes them. */
const C_TYPES = { 'long long': 'i64', 'unsigned long long': 'u64', int: 'i32', 'unsigned int': 'u32', short: 'i16', 'unsigned short': 'u16', 'signed char': 'i8', 'unsigned char': 'u8', char: 'i8', double: 'f64', float: 'f32', bool: 'bool', char32_t: 'char', 'wchar_t': 'char' };
const nxType = (t) => C_TYPES[t] || (t || '').replace(/ \*$/, '').replace(/^struct /, '');

/* Breakpoints: path -> Set of 0-based lines, kept per folder. */
const BREAKPOINTS = new Map();

function loadBreakpoints() {
  BREAKPOINTS.clear();
  const saved = S.root ? rootState(S.root).breakpoints || {} : {};
  for (const [p, lines] of Object.entries(saved)) BREAKPOINTS.set(p, new Set(lines));
  for (const d of S.docs) if (d.ed) { d.ed.breakpoints = BREAKPOINTS.get(d.path) || new Set(); d.ed.gkey = ''; d.ed.schedule(); }
}

function saveBreakpoints() {
  if (!S.root) return;
  const o = {};
  for (const [p, s] of BREAKPOINTS) if (s.size) o[p] = [...s];
  rootState(S.root).breakpoints = o;
  persist();
}

editorHooks.toggleBreakpoint = (ed, line) => {
  const doc = docOf(ed);
  if (!doc || doc.virtual) return;
  if (!BREAKPOINTS.has(doc.path)) BREAKPOINTS.set(doc.path, new Set());
  const set = BREAKPOINTS.get(doc.path);
  if (set.has(line)) set.delete(line); else set.add(line);
  ed.breakpoints = set;
  ed.gkey = '';
  ed.schedule();
  saveBreakpoints();
  if (DBG.on) {
    DBG.request('setBreakpoints', { source: { path: absPath(doc.path), name: baseName(doc.path) }, breakpoints: [...set].sort((a, b) => a - b).map((l) => ({ line: l + 1 })) });
  }
  DBG.renderAll();
};

const _openFile = openFile;
openFile = async function (path, opts) {
  const doc = await _openFile(path, opts);
  if (doc && doc.ed && !doc.ed.breakpoints) doc.ed.breakpoints = BREAKPOINTS.get(path) || new Set();
  return doc;
};

const _activate = activate;
activate = function (doc, keepFocus) {
  _activate(doc, keepFocus);
  if (S.sidebarView === 'outline') renderOutline();
  if (S.sidebarView === 'tests') renderTests();
};

function renderDebug() {
  const el = viewEl('debug');
  if (!el || S.sidebarView !== 'debug') return;
  el.innerHTML = '';
  const on = DBG.on, paused = on && DBG.frames.length > 0;
  const btn = (ic, title, onclick, enabled = true, cls = '') => h('button', { class: 'icon-btn ' + cls, title, html: icon(ic), onclick, disabled: !enabled });
  el.append(viewHead('DÉBOGAGE'));
  el.append(h('div', { class: 'dbg-toolbar' },
    on ? btn('cont', 'Continuer (F8)', () => DBG.cmd('continue'), paused) : btn('play', 'Démarrer le débogage (F9)', () => DBG.start(), true, 'dbg-start'),
    btn('stepOver', 'Pas à pas principal (F10)', () => DBG.cmd('next'), paused),
    btn('stepIn', 'Pas à pas détaillé (F11)', () => DBG.cmd('stepIn'), paused),
    btn('stepOut', 'Pas à pas sortant (Maj+F11)', () => DBG.cmd('stepOut'), paused),
    btn('stop', 'Arrêter (Maj+F9)', () => DBG.stop(), on, 'dbg-stop'),
    h('span', { id: 'dbg-status', class: 'dbg-status', text: DBG.status })));
  if (!on) {
    el.append(h('div', { class: 'sb-empty', html:
      'Cliquez dans la marge gauche d\'un fichier <code>.nx</code> pour placer des <b>points d\'arrêt</b>, puis appuyez sur <span class="kbd">F9</span>.<br><br>' +
      'Le programme s\'arrête sur chaque point d\'arrêt : vous voyez la valeur des variables et la pile d\'appels, et avancez pas à pas (F10, F11).' }));
  }
  // Variables
  const sec = (title, ...kids) => h('div', { class: 'dbg-section' }, h('div', { class: 'dbg-title', text: title }), ...kids);
  if (paused) {
    const vbox = h('div', { class: 'dbg-vars' });
    const addVars = (ref, depth) => {
      for (const v of DBG.vars.get(ref) || []) {
        const exp = v.variablesReference && DBG.expanded.has(v.variablesReference);
        const row = h('div', { class: 'dbg-var', style: { paddingLeft: 8 + depth * 14 + 'px' }, title: v.type || '' },
          h('span', { class: 'dbg-twisty', text: v.variablesReference ? (exp ? '▾' : '▸') : '' }),
          h('span', { class: 'dbg-vname', text: v.name }), h('span', { class: 'dbg-veq', text: ' = ' }), h('span', { class: 'dbg-vval', text: v.value }),
          v.type ? h('span', { class: 'dbg-vtype', text: nxType(v.type) }) : null);
        if (v.variablesReference) {
          row.style.cursor = 'pointer';
          row.onclick = async () => {
            if (DBG.expanded.has(v.variablesReference)) DBG.expanded.delete(v.variablesReference);
            else { DBG.expanded.add(v.variablesReference); await DBG.loadVars(v.variablesReference); }
            renderDebug();
          };
        }
        vbox.append(row);
        if (exp) addVars(v.variablesReference, depth + 1);
      }
    };
    for (const s of DBG.scopes) addVars(s.variablesReference, 0);
    if (!vbox.childNodes.length) vbox.append(h('div', { class: 'sb-empty', text: 'Aucune variable.' }));
    el.append(sec('VARIABLES', vbox));
    const fbox = h('div', {});
    DBG.frames.forEach((f, i) => {
      const rel = f.source && DBG.relPath(f.source.path);
      const where = f.source ? `${f.source.name || ''}:${f.line}` : '';
      fbox.append(h('div', { class: 'sb-row' + (i === DBG.frameIndex ? ' selected' : '') + (rel ? '' : ' dim'), onclick: () => DBG.selectFrame(i) },
        h('span', { class: 'sb-name', text: (f.name || '').replace(/#\d+$/, '') }), h('span', { class: 'sb-detail', text: where })));
    });
    el.append(sec("PILE D'APPELS", fbox));
  }
  // Breakpoints
  const bbox = h('div', {});
  for (const [p, set] of BREAKPOINTS) {
    for (const l of [...set].sort((a, b) => a - b)) {
      bbox.append(h('div', { class: 'sb-row', onclick: () => openFile(p, { line: l }) },
        h('span', { class: 'gbp-inline' }), h('span', { class: 'sb-name', text: baseName(p) }), h('span', { class: 'sb-detail', text: 'ligne ' + (l + 1) }),
        h('button', { class: 'icon-btn sb-inline', title: 'Supprimer', html: icon('close'), onclick: (e) => {
          e.stopPropagation();
          set.delete(l);
          const d = S.docs.find((x) => x.path === p && x.ed);
          if (d) { d.ed.gkey = ''; d.ed.schedule(); }
          saveBreakpoints();
          renderDebug();
        } })));
    }
  }
  if (!bbox.childNodes.length) bbox.append(h('div', { class: 'sb-empty', text: "Aucun point d'arrêt." }));
  el.append(sec("POINTS D'ARRÊT", bbox));
}

Object.assign(CMD, {
  debugStart: { label: () => (DBG.on ? 'Continuer' : 'Démarrer le débogage'), key: 'F9', run: () => DBG.start() },
  debugStop: { label: 'Arrêter le débogage', key: 'Maj+F9', enabled: () => DBG.on, run: () => DBG.stop() },
  debugOver: { label: 'Pas à pas principal', key: 'F10', enabled: () => DBG.on, run: () => DBG.cmd('next') },
  debugIn: { label: 'Pas à pas détaillé', key: 'F11', enabled: () => DBG.on, run: () => DBG.cmd('stepIn') },
  debugOut: { label: 'Pas à pas sortant', key: 'Maj+F11', enabled: () => DBG.on, run: () => DBG.cmd('stepOut') },
  goToDefinition: { label: 'Atteindre la définition', key: 'F12', enabled: () => isNx(S.active), run: () => editorHooks.definition(S.active.ed) },
  findReferences: { label: 'Rechercher les références', key: 'Maj+F12', enabled: () => isNx(S.active), run: () => editorHooks.references(S.active.ed) },
  renameSymbol: { label: 'Renommer le symbole', key: 'F2', enabled: () => isNx(S.active), run: () => editorHooks.rename(S.active.ed) },
  formatDocument: { label: 'Mettre en forme le document', key: 'Maj+Alt+F', enabled: () => isNx(S.active), run: () => formatDoc(S.active, true) },
  showOutline: { label: 'Structure', run: () => showView('outline') },
  showTests: { label: 'Tests', run: () => showView('tests') },
  showDebug: { label: 'Débogage', run: () => showView('debug') },
  showPackages: { label: 'Paquets', run: () => showView('packages') },
  showPlugins: { label: 'Extensions', run: () => showView('plugins') },
  toggleFormatOnSave: { label: 'Mise en forme à l\'enregistrement', checked: () => !!settings.formatOnSave, run: () => { settings.formatOnSave = !settings.formatOnSave; saveSettings(); } },
});

// Menus: new entries.
(function extendMenus() {
  const menu = (name) => MENUS.find((m) => m[0] === name)[1];
  menu('Édition').push('-', 'goToDefinition', 'findReferences', 'renameSymbol', 'formatDocument', 'toggleFormatOnSave');
  menu('Affichage').splice(6, 0, 'showOutline', 'showTests', 'showDebug', 'showPackages', 'showPlugins');
  MENUS.splice(4, 0, ['Déboguer', ['debugStart', 'debugStop', '-', 'debugOver', 'debugIn', 'debugOut', '-', 'showDebug']]);
})();

addEventListener('keydown', (e) => {
  if (e.defaultPrevented || modalStack.length) return;
  const k = e.key;
  const go = (id) => { e.preventDefault(); run(id); };
  if (k === 'F9') return go(e.shiftKey ? 'debugStop' : 'debugStart');
  if (k === 'F8' && DBG.on) return go('debugStart');
  if (k === 'F10' && DBG.on) return go('debugOver');
  if (k === 'F11' && DBG.on) return go(e.shiftKey ? 'debugOut' : 'debugIn');
}, true);

/* ==================================================================== packages (nxpm) */

async function renderPackages() {
  const el = viewEl('packages');
  if (!el || S.sidebarView !== 'packages') return;
  el.innerHTML = '';
  el.append(viewHead('PAQUETS', iconBtn('refresh', 'Actualiser', () => renderPackages())));
  const box = h('div', { class: 'sb-list pkg' });
  el.append(box);
  if (!S.root) { box.append(h('div', { class: 'sb-empty', text: 'Ouvrez un dossier.' })); return; }
  let info;
  try { info = await api.get('pkg'); } catch (e) { box.append(h('div', { class: 'sb-empty', text: e.message })); return; }
  if (!info.project) {
    box.append(h('div', { class: 'sb-empty', html: 'Ce dossier n\'est pas encore un projet NEXUL.<br><br>Un projet est décrit par <code>nexul.toml</code> : son nom, sa version et ses dépendances, dont les versions exactes et l\'empreinte SHA-256 sont figées dans <code>nexul.lock</code>.' }));
    box.append(h('button', { class: 'btn primary block', text: 'Initialiser le projet (nexul.toml)', onclick: () => pkgRun({ cmd: 'init' }) }));
    return;
  }
  box.append(h('div', { class: 'pkg-head' }, h('b', { text: info.name }), h('span', { text: ' ' + info.version })));
  box.append(h('div', { class: 'sb-title', text: 'DÉPENDANCES' }));
  if (!info.dependencies.length) box.append(h('div', { class: 'sb-empty', text: 'Aucune dépendance.' }));
  for (const d of info.dependencies) {
    const commit = (d.locked.split('#')[1] || '').slice(0, 10);
    box.append(h('div', { class: 'pkg-dep', title: d.checksum || 'non installé' },
      h('div', { class: 'pkg-name' }, iconEl('box'), h('b', { text: d.name }), h('span', { class: 'pkg-kind', text: d.kind === 'git' ? 'Git' : 'dossier' }),
        h('button', { class: 'icon-btn sb-inline', title: 'Retirer cette dépendance', html: icon('trash'), onclick: async () => {
          if (await confirmBox(`Retirer ${d.name} ?`, `La dépendance « ${d.name} » sera retirée de nexul.toml.`, 'Retirer', true)) pkgRun({ cmd: 'remove', name: d.name });
        } })),
      h('div', { class: 'pkg-loc', text: d.location + (commit ? ' · ' + commit : '') }),
      h('div', { class: 'pkg-sum', text: d.checksum ? '🔒 ' + d.checksum.slice(0, 23) + '…' : 'non installé' })));
  }
  box.append(h('div', { class: 'pkg-actions' },
    h('button', { class: 'btn primary', text: 'Ajouter…', onclick: () => pkgAdd() }),
    h('button', { class: 'btn', text: 'Installer', title: 'Récupère les dépendances aux versions de nexul.lock', onclick: () => pkgRun({ cmd: 'install' }) }),
    h('button', { class: 'btn', text: 'Mettre à jour', title: 'Passe les dépendances Git à leur dernière version', onclick: () => pkgRun({ cmd: 'update' }) }),
    h('button', { class: 'btn', text: 'Vérifier', title: 'Vérifie l\'intégrité (SHA-256) des paquets', onclick: () => pkgRun({ cmd: 'verify' }) })));
}

async function pkgRun(params) {
  if (S.run) await stopRun(true);
  showPanel('terminal');
  termClear();
  let r;
  try { r = await api.post('pkg?' + qs(params)); } catch (e) { termWrite(`\x1b[31m${e.message}\x1b[0m\n`); return; }
  S.run = { id: r.id, from: 0, kind: 'build', path: 'nexul.toml', started: performance.now(), pkg: true };
  updateRunUi();
  pollRun(S.run);
  const wait = setInterval(() => { if (!S.run || !S.run.pkg) { clearInterval(wait); refreshTree(); renderPackages(); } }, 300);
}

async function pkgAdd() {
  const name = h('input', { class: 'input', placeholder: 'nom du paquet (ex. : geometrie)', spellcheck: 'false' });
  const kind = h('select', { class: 'input' }, h('option', { value: 'path', text: 'Dossier local' }), h('option', { value: 'git', text: 'Dépôt Git' }));
  const loc = h('input', { class: 'input', placeholder: '../geometrie', spellcheck: 'false' });
  const tag = h('input', { class: 'input', placeholder: 'étiquette (ex. : v1.0) — facultatif', spellcheck: 'false' });
  const tagRow = h('label', { class: 'field', style: { display: 'none' } }, h('span', { text: 'Version (étiquette Git)' }), tag);
  kind.onchange = () => { loc.placeholder = kind.value === 'git' ? 'https://github.com/auteur/paquet' : '../geometrie'; tagRow.style.display = kind.value === 'git' ? '' : 'none'; };
  const body = h('div', { class: 'form' },
    h('label', { class: 'field' }, h('span', { text: 'Nom' }), name),
    h('label', { class: 'field' }, h('span', { text: 'Source' }), kind),
    h('label', { class: 'field' }, h('span', { text: 'Emplacement' }), loc), tagRow,
    h('div', { class: 'hint', text: 'Le paquet est importé avec « import nom » (fichier src/lib.nx) ou « import nom.module ».' }));
  openModal({
    title: 'Ajouter une dépendance', icon: 'box', body,
    buttons: [
      { label: 'Annuler', onclick: (s) => s.close() },
      { label: 'Ajouter', primary: true, onclick: (s) => {
        if (!/^[A-Za-z_]\w*$/.test(name.value)) { toast('Nom invalide : lettres, chiffres et _.', 'error'); return; }
        if (!loc.value.trim()) { toast("Indiquez l'emplacement du paquet.", 'error'); return; }
        s.close();
        const p = { cmd: 'add', name: name.value };
        if (kind.value === 'git') { p.git = loc.value.trim(); if (tag.value.trim()) p.tag = tag.value.trim(); } else p.path = loc.value.trim();
        pkgRun(p);
      } },
    ],
  });
  setTimeout(() => name.focus(), 0);
}

/* ==================================================================== status bar & start */

const _updateStatus = updateStatus;
updateStatus = function () {
  _updateStatus();
  const sb = $('#statusbar');
  if (!sb) return;
  let el = $('#sb-debug');
  if (!el) { el = h('span', { id: 'sb-debug', class: 'sb-item' }); sb.prepend(el); }
  el.textContent = DBG.on ? '🐞 ' + (DBG.status || 'Débogage') : '';
  el.style.display = DBG.on ? '' : 'none';
};

const _applyState = applyState;
applyState = function (st) {
  const before = S.root;
  const r = _applyState(st);
  if (S.root !== before) { loadBreakpoints(); if (S.sidebarView === 'packages') renderPackages(); }
  return r;
};

// Refresh the outline after edits.
const _change = editorHooks.change;
editorHooks.change = function (ed) {
  _change(ed);
  if (S.sidebarView === 'outline' && S.active && S.active.ed === ed) {
    clearTimeout(DBG.outlineTimer);
    DBG.outlineTimer = setTimeout(renderOutline, 800);
  }
};

/* The views are added when the workbench is built (after the settings are loaded). */
const _buildLayout = buildLayout;
buildLayout = function () {
  _buildLayout();
  installViews();
  loadBreakpoints();
  if (IDE_VIEWS.some((v) => v[0] === S.sidebarView) && settings.sidebarOpen) showView(S.sidebarView, false);
};
