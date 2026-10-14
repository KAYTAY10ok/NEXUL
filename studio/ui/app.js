'use strict';
/* NEXUL Studio — the workbench: explorer, tabs, panels, live preview, commands. */

/* ==================================================================== helpers */

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

function h(tag, props, ...kids) {
  const el = document.createElement(tag);
  if (props) {
    for (const k in props) {
      const v = props[k];
      if (v === undefined || v === null || v === false) continue;
      if (k === 'class') el.className = v;
      else if (k === 'text') el.textContent = v;
      else if (k === 'html') el.innerHTML = v;
      else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
      else if (k === 'dataset') Object.assign(el.dataset, v);
      else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
      else el.setAttribute(k, v === true ? '' : v);
    }
  }
  for (const c of kids.flat(3)) if (c !== null && c !== undefined && c !== false) el.append(c.nodeType ? c : String(c));
  return el;
}

const ICON_PATHS = {
  files: '<path d="M15 3H8a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h9a2 2 0 0 0 2-2V7z"/><path d="M15 3v4h4"/><path d="M6 7H5a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2v-1"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  web: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>',
  play: '<path d="M7 4.6v14.8a1 1 0 0 0 1.5.9l11.8-7.4a1 1 0 0 0 0-1.7L8.5 3.7A1 1 0 0 0 7 4.6z" fill="currentColor" stroke="none"/>',
  stop: '<rect x="6" y="6" width="12" height="12" rx="2" fill="currentColor" stroke="none"/>',
  check: '<path d="M12 3 4.5 6v5.5c0 4.4 3.2 8.2 7.5 9.5 4.3-1.3 7.5-5.1 7.5-9.5V6z"/><path d="m9 12 2.2 2.2L15.5 10"/>',
  live: '<circle cx="12" cy="12" r="2.2"/><path d="M16.2 7.8a6 6 0 0 1 0 8.4M7.8 16.2a6 6 0 0 1 0-8.4M19 5a10 10 0 0 1 0 14M5 19A10 10 0 0 1 5 5"/>',
  close: '<path d="M17 7 7 17M7 7l10 10"/>',
  chevronRight: '<path d="m9 6 6 6-6 6"/>',
  chevronDown: '<path d="m6 9 6 6 6-6"/>',
  newFile: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M12 11v6M9 14h6"/>',
  newFolder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M12 10v6M9 13h6"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" fill="currentColor" fill-opacity=".18"/>',
  folderOpen: '<path d="M4 19h13.4a2 2 0 0 0 1.9-1.4L21.5 11H7.6a2 2 0 0 0-1.9 1.4z" fill="currentColor" fill-opacity=".18"/><path d="M4 19a1 1 0 0 1-1-1V7a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v2"/>',
  file: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>',
  image: '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-5-5L5 21"/>',
  refresh: '<path d="M20 11a8 8 0 0 0-14.3-4.9L4 8"/><path d="M4 4v4h4M4 13a8 8 0 0 0 14.3 4.9L20 16"/><path d="M20 20v-4h-4"/>',
  collapse: '<path d="m7 11 5-5 5 5M7 18l5-5 5 5"/>',
  error: '<circle cx="12" cy="12" r="9"/><path d="m15 9-6 6M9 9l6 6"/>',
  warning: '<path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4M12 17h.01"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 16v-4M12 8h.01"/>',
  ok: '<circle cx="12" cy="12" r="9"/><path d="m8 12.5 2.8 2.8L16 9.8"/>',
  terminal: '<path d="m5 17 5-5-5-5M12 19h7"/>',
  trash: '<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>',
  external: '<path d="M14 4h6v6M10 14 20 4M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>',
  desktop: '<rect x="2" y="4" width="20" height="13" rx="2"/><path d="M8 21h8M12 17v4"/>',
  tablet: '<rect x="5" y="2" width="14" height="20" rx="2"/><path d="M11 18h2"/>',
  phone: '<rect x="7" y="2" width="10" height="20" rx="2.5"/><path d="M11 18h2"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  moon: '<path d="M20.5 13.4A8.5 8.5 0 1 1 10.6 3.5a6.6 6.6 0 0 0 9.9 9.9z"/>',
  hammer: '<path d="m14 7 3 3M5 20l9-9M11.5 5.5l3-3 7 7-3 3z"/>',
  flask: '<path d="M9 3h6M10 3v6.5L4.6 18.6A1.6 1.6 0 0 0 6 21h12a1.6 1.6 0 0 0 1.4-2.4L14 9.5V3"/><path d="M7.5 15h9"/>',
  book: '<path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20V3H6.5A2.5 2.5 0 0 0 4 5.5z"/><path d="M4 19.5A2.5 2.5 0 0 0 6.5 22H20v-5"/>',
  keyboard: '<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/>',
  sparkle: '<path d="M12 3.5 13.8 9l5.7 1.9-5.7 1.9L12 18.5l-1.8-5.7-5.7-1.9L10.2 9z"/><path d="M19 3v4M17 5h4"/>',
  copy: '<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/>',
  edit: '<path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/>',
  palette: '<path d="M12 2a10 10 0 0 0 0 20 2 2 0 0 0 2-2v-.5a2 2 0 0 1 2-2h2a4 4 0 0 0 4-4C22 6.5 17.5 2 12 2z"/><circle cx="7.5" cy="11.5" r="1.3" fill="currentColor"/><circle cx="10" cy="7" r="1.3" fill="currentColor"/><circle cx="15" cy="7" r="1.3" fill="currentColor"/>',
  layout: '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M9 21V9"/>',
  code: '<path d="m16 18 6-6-6-6M8 6l-6 6 6 6"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  command: '<path d="M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z"/>',
  wand: '<path d="m15 4 1 2 2 1-2 1-1 2-1-2-2-1 2-1zM5 19 16 8"/>',
};

function icon(name, cls) {
  return `<span class="icon${cls ? ' ' + cls : ''}"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${ICON_PATHS[name] || ''}</svg></span>`;
}

function iconEl(name, cls) {
  const t = document.createElement('template');
  t.innerHTML = icon(name, cls);
  return t.content.firstChild;
}

const FILE_BADGES = {
  nx: ['N', 'linear-gradient(135deg,#8b5cf6,#22d3ee)', '#fff'],
  html: ['<>', '#e44d26', '#fff'], htm: ['<>', '#e44d26', '#fff'],
  css: ['#', '#2965f1', '#fff'],
  js: ['JS', '#f0d43a', '#26262b'], mjs: ['JS', '#f0d43a', '#26262b'],
  json: ['{}', '#7c8296', '#fff'],
  md: ['M', '#3d8bd3', '#fff'],
  txt: ['≡', '#6b7280', '#fff'],
  exe: ['▶', '#475569', '#fff'],
  c: ['C', '#5c6bc0', '#fff'], h: ['H', '#7e57c2', '#fff'], rs: ['RS', '#c4532f', '#fff'], py: ['PY', '#3572a5', '#fff'],
};
const IMAGE_EXT = new Set(['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico', 'bmp']);

function fileIconEl(name, dir, open) {
  if (dir) {
    const el = iconEl(open ? 'folderOpen' : 'folder');
    el.style.color = '#e5a948';
    return el;
  }
  const ext = extOf(name);
  if (IMAGE_EXT.has(ext)) {
    const el = iconEl('image');
    el.style.color = '#3fb27f';
    return el;
  }
  const b = FILE_BADGES[ext];
  if (!b) {
    const el = iconEl('file');
    el.style.color = 'var(--fg-muted)';
    return el;
  }
  return h('span', { class: 'fi', style: { background: b[1], color: b[2] }, text: b[0] });
}

const baseName = (p) => p.slice(p.lastIndexOf('/') + 1);
const dirName = (p) => { const i = p.lastIndexOf('/'); return i < 0 ? '' : p.slice(0, i); };
const joinRel = (a, b) => (a ? a + '/' + b : b);
const extOf = (p) => { const n = baseName(p); const i = n.lastIndexOf('.'); return i <= 0 ? '' : n.slice(i + 1).toLowerCase(); };
const stemOf = (p) => { const n = baseName(p); const i = n.lastIndexOf('.'); return i <= 0 ? n : n.slice(0, i); };
const qs = (o) => Object.entries(o).map(([k, v]) => k + '=' + encodeURIComponent(v)).join('&');
const livePath = (rel) => '/live/' + rel.split('/').map(encodeURIComponent).join('/');

function normRel(p) {
  const out = [];
  for (const seg of p.replace(/\\/g, '/').split('/')) {
    if (!seg || seg === '.') continue;
    if (seg === '..') out.pop(); else out.push(seg);
  }
  return out.join('/');
}

function absPath(rel) { return S.root + (rel ? S.sep + rel.replace(/\//g, S.sep) : ''); }

function toast(msg, kind = 'info', ms = 4000) {
  let root = $('.toasts');
  if (!root) { root = h('div', { class: 'toasts' }); document.body.append(root); }
  const ic = kind === 'error' ? icon('error', 'ic-err') : kind === 'success' ? icon('ok', 'ic-ok') : icon('info');
  const t = h('div', { class: 'toast ' + kind, html: ic + `<div>${escHtml(msg)}</div>` });
  root.append(t);
  setTimeout(() => {
    t.style.transition = 'opacity .25s, transform .25s';
    t.style.opacity = '0';
    t.style.transform = 'translateX(12px)';
    setTimeout(() => t.remove(), 260);
  }, ms);
}

const api = {
  async req(method, path, body) {
    const headers = { 'X-Nexul-Token': window.NX_TOKEN };
    let data = body;
    if (body !== undefined && typeof body !== 'string') { data = JSON.stringify(body); headers['Content-Type'] = 'application/json'; }
    const r = await fetch('/api/' + path, { method, headers, body: data, cache: 'no-store' });
    const ct = r.headers.get('content-type') || '';
    const out = ct.includes('json') ? await r.json() : await r.text();
    if (!r.ok) throw new Error((out && out.error) || String(out) || r.statusText);
    return out;
  },
  get(path) { return this.req('GET', path); },
  post(path, body) { return this.req('POST', path, body === undefined ? '' : body); },
};

/* ==================================================================== state & settings */

const S = {
  root: null, rootName: null, nxc: null, port: 0, sep: '\\',
  tree: [], treeJson: '', files: [],
  expanded: new Set(), selected: null, inline: null,
  docs: [], active: null,
  checks: new Map(), fileDiags: new Map(), checkTimers: new Map(),
  run: null,
  live: { on: false, path: null, device: 'desktop' },
  sidebarView: 'explorer',
  searchCase: false,
};

const DEFAULTS = {
  theme: 'dark', fontSize: 14, tabSize: 4, autoSave: true, minimap: true, wordHighlight: true, autoClose: true, checkOnSave: true,
  recent: [], lastRoot: null, lastBrowse: '', projectDir: '', sidebarW: 270, panelH: 230, previewW: 0, args: '', panelOpen: true, sidebarOpen: true,
};

const settings = { ...DEFAULTS };

/* Settings and per-folder sessions are kept by the server (%APPDATA%\NEXUL Studio\state.json). */
const KV = { data: { settings: {}, roots: {} }, timer: 0 };

async function loadKv() {
  try {
    const d = await api.get('kv');
    KV.data = { settings: d.settings || {}, roots: d.roots || {} };
  } catch (e) { /* first start */ }
  Object.assign(settings, KV.data.settings);
}

function persist() {
  clearTimeout(KV.timer);
  KV.timer = setTimeout(flushKv, 400);
}

function flushKv(keepalive) {
  clearTimeout(KV.timer);
  KV.timer = 0;
  const roots = Object.entries(KV.data.roots).sort((a, b) => (b[1].t || 0) - (a[1].t || 0)).slice(0, 30);
  KV.data.roots = Object.fromEntries(roots);
  const body = JSON.stringify(KV.data);
  if (keepalive) fetch('/api/kv', { method: 'POST', keepalive: true, headers: { 'X-Nexul-Token': window.NX_TOKEN }, body }).catch(() => {});
  else api.post('kv', body).catch(() => {});
}

addEventListener('pagehide', () => { if (KV.timer) flushKv(true); });

function saveSettings() { KV.data.settings = { ...settings }; persist(); }

function rootState(root) {
  if (!root) return {};
  const r = KV.data.roots[root] || (KV.data.roots[root] = {});
  r.t = Date.now();
  return r;
}

function applySettings() {
  const de = document.documentElement;
  de.dataset.theme = settings.theme;
  de.style.setProperty('--code-size', settings.fontSize + 'px');
  de.style.setProperty('--line-h', Math.round(settings.fontSize * 1.5) + 'px');
  Object.assign(Editor.config, { tabSize: settings.tabSize, minimap: settings.minimap, wordHighlight: settings.wordHighlight, autoClose: settings.autoClose });
  Editor.measure();
  Editor.refreshColors();
  for (const d of S.docs) {
    if (!d.ed) continue;
    d.ed.tkey = d.ed.gkey = d.ed.mmKey = '';
    d.ed.gw = 0;
    d.ed.schedule();
  }
  updateStatus();
}

/* ==================================================================== layout */

function buildLayout() {
  // Title bar
  $('#btn-check').innerHTML = icon('check') + '<span>Vérifier</span>';
  $('#btn-run').innerHTML = icon('play') + '<span>Exécuter</span>';
  $('#btn-live').innerHTML = icon('live') + '<span>Aperçu</span>';
  $('#btn-check').onclick = () => run('check');
  $('#btn-run').onclick = () => run(S.run ? 'stop' : 'run');
  $('#btn-live').onclick = () => run('togglePreview');
  $('.cc-icon').innerHTML = icon('search');
  $('#command-center').onclick = () => openPalette('files');
  const mb = $('#menubar');
  for (const [name, items] of MENUS) {
    const b = h('button', { class: 'menu-btn', text: name });
    const open = () => {
      const r = b.getBoundingClientRect();
      showMenu(items.map(menuItem), r.left, r.bottom + 4, b);
    };
    b.addEventListener('mousedown', (e) => {
      e.preventDefault();
      if (menuState && menuState.btn === b) closeMenus(); else open();
    });
    b.addEventListener('mouseenter', () => { if (menuState && menuState.btn && menuState.btn !== b) open(); });
    mb.append(b);
  }

  // Activity bar
  const ab = $('#activitybar');
  const act = (view, ic, title) => h('button', { class: 'act-btn', dataset: { view }, title, html: icon(ic), onclick: () => showView(view, true) });
  ab.append(
    act('explorer', 'files', 'Explorateur (Ctrl+Maj+E)'),
    act('search', 'search', 'Rechercher (Ctrl+Maj+F)'),
    act('web', 'web', 'Concepteur web'),
    h('div', { class: 'act-spacer' }),
    h('button', { class: 'act-btn', id: 'act-theme', title: 'Basculer le thème clair / sombre', onclick: () => run('toggleTheme') }),
    h('button', { class: 'act-btn', title: 'Paramètres (Ctrl+,)', html: icon('gear'), onclick: () => run('settings') }),
  );

  buildSearchView();
  buildPanel();
  buildPreview();
  buildStatusBar();

  // Sizes
  $('#sidebar').style.width = settings.sidebarW + 'px';
  $('#panel').style.height = settings.panelH + 'px';
  if (settings.previewW) $('#preview').style.width = settings.previewW + 'px';
  if (!settings.sidebarOpen) { $('#sidebar').classList.add('hidden'); $('#sidebar-resizer').classList.add('hidden'); }
  if (!settings.panelOpen) { $('#panel').classList.add('hidden'); $('#panel-resizer').classList.add('hidden'); }
  $('#preview').classList.add('hidden');
  $('#preview-resizer').classList.add('hidden');

  makeResizer($('#sidebar-resizer'), (e) => {
    const w = clamp(e.clientX - $('#sidebar').getBoundingClientRect().left, 170, 640);
    $('#sidebar').style.width = w + 'px';
    settings.sidebarW = w;
  });
  makeResizer($('#panel-resizer'), (e) => {
    const wb = $('#workbench').getBoundingClientRect();
    const hgt = clamp(wb.bottom - e.clientY, 80, wb.height - 120);
    $('#panel').style.height = hgt + 'px';
    settings.panelH = hgt;
  });
  makeResizer($('#preview-resizer'), (e) => {
    const ta = $('#top-area').getBoundingClientRect();
    const w = clamp(ta.right - e.clientX, 260, ta.width - 240);
    $('#preview').style.width = w + 'px';
    settings.previewW = w;
  });

  $('#editor-container').addEventListener('wheel', (e) => {
    if (!e.ctrlKey) return;
    e.preventDefault();
    run(e.deltaY < 0 ? 'zoomIn' : 'zoomOut');
  }, { passive: false });

  const wasOpen = settings.sidebarOpen;
  showView(S.sidebarView, false);
  if (!wasOpen) setSidebar(false);
}

function makeResizer(el, onMove) {
  el.addEventListener('mousedown', (e) => {
    e.preventDefault();
    el.classList.add('dragging');
    document.body.classList.add('resizing');
    document.body.style.cursor = getComputedStyle(el).cursor;
    const up = () => {
      el.classList.remove('dragging');
      document.body.classList.remove('resizing');
      document.body.style.cursor = '';
      removeEventListener('mousemove', onMove);
      removeEventListener('mouseup', up);
      saveSettings();
    };
    addEventListener('mousemove', onMove);
    addEventListener('mouseup', up);
  });
}

function showView(view, fromActivity) {
  const sb = $('#sidebar');
  if (fromActivity && S.sidebarView === view && !sb.classList.contains('hidden')) {
    setSidebar(false);
    return;
  }
  S.sidebarView = view;
  setSidebar(true);
  for (const v of $$('.sb-view')) v.classList.toggle('active', v.dataset.view === view);
  for (const b of $$('.act-btn[data-view]')) b.classList.toggle('active', b.dataset.view === view);
  if (view === 'explorer') renderTree();
  if (view === 'web') renderWebView();
  if (view === 'search') setTimeout(() => $('#search-input').focus(), 0);
}

function setSidebar(open) {
  $('#sidebar').classList.toggle('hidden', !open);
  $('#sidebar-resizer').classList.toggle('hidden', !open);
  for (const b of $$('.act-btn[data-view]')) b.classList.toggle('active', open && b.dataset.view === S.sidebarView);
  settings.sidebarOpen = open;
  saveSettings();
}

/* ==================================================================== menus */

let menuState = null;

function closeMenus() {
  if (!menuState) return;
  menuState.el.remove();
  if (menuState.btn) menuState.btn.classList.remove('open');
  menuState = null;
}

function showMenu(items, x, y, btn) {
  closeMenus();
  const el = h('div', { class: 'menu' });
  for (const it of items) {
    if (it === '-') { el.append(h('div', { class: 'menu-sep' })); continue; }
    const row = h('div', { class: 'menu-item' + (it.disabled ? ' disabled' : '') },
      h('span', { class: 'menu-check', text: it.checked ? '✓' : '' }),
      h('span', { text: it.label }),
      it.key ? h('span', { class: 'shortcut', text: it.key }) : null);
    row.addEventListener('mousedown', (e) => e.preventDefault());
    row.addEventListener('click', () => { closeMenus(); it.run(); });
    el.append(row);
  }
  document.body.append(el);
  const r = el.getBoundingClientRect();
  el.style.left = clamp(x, 4, innerWidth - r.width - 4) + 'px';
  el.style.top = (y + r.height > innerHeight - 4 ? Math.max(4, y - r.height) : y) + 'px';
  menuState = { el, btn };
  if (btn) btn.classList.add('open');
}

document.addEventListener('mousedown', (e) => {
  if (menuState && !menuState.el.contains(e.target) && !(menuState.btn && menuState.btn.contains(e.target))) closeMenus();
}, true);
addEventListener('blur', closeMenus);

function menuItem(id) {
  if (id === '-') return '-';
  const c = CMD[id];
  return {
    label: typeof c.label === 'function' ? c.label() : c.label,
    key: c.key,
    run: () => run(id),
    checked: c.checked ? c.checked() : false,
    disabled: c.enabled ? !c.enabled() : false,
  };
}

/* ==================================================================== commands */

const hasEditor = () => !!(S.active && S.active.ed);
const edCmd = (name) => { if (hasEditor()) S.active.ed.command(name); };

const CMD = {
  newFile: { label: 'Nouveau fichier', key: 'Ctrl+N', run: () => startInlineNew('file') },
  newFolder: { label: 'Nouveau dossier', run: () => startInlineNew('dir') },
  newProject: { label: 'Nouveau projet…', run: () => newProject() },
  newSite: { label: 'Créer un site web (HTML, CSS, JS)', run: () => createSite() },
  openFolder: { label: 'Ouvrir un dossier…', key: 'Ctrl+O', run: async () => { const p = await pickFolder(); if (p) openFolder(p); } },
  openRecent: { label: 'Ouvrir un dossier récent…', run: () => openPalette('recent') },
  openExamples: { label: 'Ouvrir les exemples NEXUL', run: () => { const p = examplesDir(); if (p) openFolder(p); else toast('Exemples introuvables.', 'error'); } },
  closeFolder: { label: 'Fermer le dossier', enabled: () => !!S.root, run: () => closeFolder() },
  save: { label: 'Enregistrer', key: 'Ctrl+S', enabled: hasEditor, run: () => S.active && saveDoc(S.active, true) },
  saveAll: { label: 'Tout enregistrer', key: 'Ctrl+Maj+S', run: () => saveAll() },
  closeEditor: { label: "Fermer l'éditeur", key: 'Ctrl+W', enabled: () => !!S.active, run: () => S.active && closeDoc(S.active) },
  closeAll: { label: 'Fermer tous les éditeurs', enabled: () => S.docs.length > 0, run: () => closeAllDocs() },
  settings: { label: 'Paramètres', key: 'Ctrl+,', run: () => openSettings() },
  undo: { label: 'Annuler', key: 'Ctrl+Z', enabled: hasEditor, run: () => edCmd('undo') },
  redo: { label: 'Rétablir', key: 'Ctrl+Y', enabled: hasEditor, run: () => edCmd('redo') },
  cut: { label: 'Couper', key: 'Ctrl+X', enabled: hasEditor, run: () => edCmd('cut') },
  copy: { label: 'Copier', key: 'Ctrl+C', enabled: hasEditor, run: () => edCmd('copy') },
  paste: { label: 'Coller', key: 'Ctrl+V', enabled: hasEditor, run: () => edCmd('paste') },
  selectAll: { label: 'Tout sélectionner', key: 'Ctrl+A', enabled: hasEditor, run: () => edCmd('selectAll') },
  find: { label: 'Rechercher', key: 'Ctrl+F', enabled: hasEditor, run: () => edCmd('find') },
  replace: { label: 'Remplacer', key: 'Ctrl+H', enabled: hasEditor, run: () => edCmd('replace') },
  findInFiles: { label: 'Rechercher dans les fichiers', key: 'Ctrl+Maj+F', run: () => showView('search') },
  comment: { label: 'Commenter / décommenter', key: 'Ctrl+/', enabled: hasEditor, run: () => edCmd('comment') },
  duplicate: { label: 'Dupliquer la ligne', key: 'Maj+Alt+↓', enabled: hasEditor, run: () => edCmd('duplicate') },
  moveUp: { label: 'Monter la ligne', key: 'Alt+↑', enabled: hasEditor, run: () => edCmd('moveUp') },
  moveDown: { label: 'Descendre la ligne', key: 'Alt+↓', enabled: hasEditor, run: () => edCmd('moveDown') },
  deleteLine: { label: 'Supprimer la ligne', key: 'Ctrl+Maj+K', enabled: hasEditor, run: () => edCmd('deleteLine') },
  indent: { label: 'Indenter', key: 'Tab', enabled: hasEditor, run: () => edCmd('indent') },
  outdent: { label: 'Désindenter', key: 'Maj+Tab', enabled: hasEditor, run: () => edCmd('outdent') },
  suggest: { label: 'Afficher les suggestions', key: 'Ctrl+Espace', enabled: hasEditor, run: () => edCmd('suggest') },
  gotoLine: { label: 'Aller à la ligne…', key: 'Ctrl+G', enabled: hasEditor, run: () => openPalette('line') },
  palette: { label: 'Palette de commandes', key: 'Ctrl+Maj+P', run: () => openPalette('commands') },
  quickOpen: { label: 'Ouvrir un fichier…', key: 'Ctrl+P', run: () => openPalette('files') },
  showExplorer: { label: 'Explorateur', key: 'Ctrl+Maj+E', run: () => showView('explorer') },
  showSearch: { label: 'Recherche', run: () => showView('search') },
  showWeb: { label: 'Concepteur web', run: () => showView('web') },
  toggleSidebar: { label: 'Barre latérale', key: 'Ctrl+B', checked: () => !$('#sidebar').classList.contains('hidden'), run: () => setSidebar($('#sidebar').classList.contains('hidden')) },
  toggleTerminal: { label: 'Terminal', key: 'Ctrl+ù', checked: () => panelOpen() && S.panelTab === 'terminal', run: () => togglePanel('terminal') },
  showProblems: { label: 'Problèmes', key: 'Ctrl+Maj+M', checked: () => panelOpen() && S.panelTab === 'problems', run: () => togglePanel('problems') },
  togglePreview: { label: 'Aperçu web en direct', key: 'Alt+L', checked: () => S.live.on, run: () => (S.live.on ? stopLive() : startLive()) },
  openExternal: { label: "Ouvrir l'aperçu dans le navigateur", run: () => openExternal() },
  toggleMinimap: { label: 'Minimap', checked: () => settings.minimap, run: () => { settings.minimap = !settings.minimap; saveSettings(); applySettings(); } },
  toggleTheme: { label: () => (settings.theme === 'dark' ? 'Thème clair' : 'Thème sombre'), run: () => { settings.theme = settings.theme === 'dark' ? 'light' : 'dark'; saveSettings(); applySettings(); renderActivityTheme(); } },
  zoomIn: { label: 'Agrandir le texte', key: 'Ctrl+=', run: () => setFontSize(settings.fontSize + 1) },
  zoomOut: { label: 'Réduire le texte', key: 'Ctrl+-', run: () => setFontSize(settings.fontSize - 1) },
  zoomReset: { label: 'Taille de texte par défaut', key: 'Ctrl+0', run: () => setFontSize(14) },
  run: { label: 'Exécuter', key: 'F5', run: () => runProgram('run') },
  stop: { label: 'Arrêter', key: 'Maj+F5', enabled: () => !!S.run, run: () => stopRun() },
  check: { label: 'Vérifier le programme', key: 'F7', run: () => checkCommand() },
  test: { label: 'Lancer les tests', key: 'F6', run: () => runProgram('test') },
  build: { label: 'Compiler en exécutable (.exe)', key: 'Ctrl+Maj+B', run: () => runProgram('build') },
  args: { label: 'Arguments du programme…', run: () => editArgs() },
  welcome: { label: "Page d'accueil", run: () => { closeAllDocs(); } },
  shortcuts: { label: 'Raccourcis clavier', run: () => showShortcuts() },
  explainCode: { label: "Expliquer un code d'erreur…", run: async () => { const c = await promptBox("Expliquer un code d'erreur", { label: 'Code', placeholder: 'E0301' }); if (c) explain(c.toUpperCase()); } },
  revealFile: { label: "Afficher dans l'Explorateur Windows", enabled: () => !!S.active, run: () => S.active && api.post('reveal?' + qs({ path: S.active.path })) },
  about: { label: 'À propos de NEXUL Studio', run: () => showAbout() },
};

function run(id) {
  closeMenus();
  const c = CMD[id];
  if (!c) return;
  if (c.enabled && !c.enabled()) return;
  Promise.resolve(c.run()).catch((e) => toast(e.message || String(e), 'error'));
}

const MENUS = [
  ['Fichier', ['newFile', 'newFolder', 'newProject', 'newSite', '-', 'openFolder', 'openRecent', 'closeFolder', '-', 'save', 'saveAll', '-', 'closeEditor', 'closeAll', '-', 'settings']],
  ['Édition', ['undo', 'redo', '-', 'cut', 'copy', 'paste', 'selectAll', '-', 'find', 'replace', 'findInFiles', '-', 'comment', 'duplicate', 'moveUp', 'moveDown', 'deleteLine', '-', 'suggest', 'gotoLine']],
  ['Affichage', ['palette', 'quickOpen', '-', 'showExplorer', 'showSearch', 'showWeb', 'toggleSidebar', '-', 'toggleTerminal', 'showProblems', 'togglePreview', '-', 'toggleMinimap', 'toggleTheme', '-', 'zoomIn', 'zoomOut', 'zoomReset']],
  ['Exécuter', ['run', 'stop', '-', 'check', 'test', 'build', '-', 'args', '-', 'togglePreview', 'openExternal']],
  ['Aide', ['welcome', 'shortcuts', 'explainCode', 'openExamples', '-', 'about']],
];

function setFontSize(n) {
  settings.fontSize = clamp(n, 10, 28);
  saveSettings();
  applySettings();
}

function renderActivityTheme() {
  const b = $('#act-theme');
  if (b) b.innerHTML = icon(settings.theme === 'dark' ? 'sun' : 'moon');
}

function examplesDir() {
  const m = S.nxc && /^(.*)[\\/]target[\\/](?:release|debug)[\\/]nxc(?:\.exe)?$/i.exec(S.nxc);
  return m ? m[1] + '\\examples' : null;
}

/* ==================================================================== keyboard */

addEventListener('keydown', (e) => {
  if (e.defaultPrevented) return;
  const k = e.key;
  const lk = k.length === 1 ? k.toLowerCase() : k;
  const ctrl = e.ctrlKey || e.metaKey;
  if (k === 'Escape') {
    if (paletteState) { closePalette(); e.preventDefault(); return; }
    if (modalStack.length) { cancelTopModal(); e.preventDefault(); return; }
    if (menuState) { closeMenus(); e.preventDefault(); return; }
    return;
  }
  if (modalStack.length || paletteState) return;
  if (e.ctrlKey && e.altKey) return; // AltGr
  const go = (id) => { e.preventDefault(); run(id); };
  if (k === 'F5') return go(e.shiftKey ? 'stop' : 'run');
  if (k === 'F6') return go('test');
  if (k === 'F7') return go('check');
  if (k === 'F1') return go('palette');
  if (k === 'F2' && document.activeElement === $('#tree') && S.selected) { e.preventDefault(); startRename(S.selected); return; }
  if (k === 'Delete' && document.activeElement === $('#tree') && S.selected) { e.preventDefault(); deletePath(S.selected); return; }
  if (e.altKey && !ctrl && lk === 'l') return go('togglePreview');
  if (!ctrl) return;
  if (lk === 's') return go(e.shiftKey ? 'saveAll' : 'save');
  if (lk === 'p') return go(e.shiftKey ? 'palette' : 'quickOpen');
  if (lk === 'o' && !e.shiftKey) return go('openFolder');
  if (lk === 'n' && !e.shiftKey) return go('newFile');
  if (lk === 'w' || k === 'F4') return go('closeEditor');
  if (lk === 'b') return go(e.shiftKey ? 'build' : 'toggleSidebar');
  if (lk === 'j' || k === '`' || k === 'ù' || e.code === 'Backquote') return go('toggleTerminal');
  if (lk === 'f' && e.shiftKey) return go('findInFiles');
  if (lk === 'f') return go('find');
  if (lk === 'h' && !e.shiftKey) return go('replace');
  if (lk === 'e' && e.shiftKey) return go('showExplorer');
  if (lk === 'm' && e.shiftKey) return go('showProblems');
  if (lk === 'g') return go('gotoLine');
  if (k === ',') return go('settings');
  if (k === '=' || k === '+' || e.code === 'NumpadAdd') return go('zoomIn');
  if (k === '-' || k === ')' || e.code === 'NumpadSubtract') return go('zoomOut');
  if (k === '0' || k === 'à' || e.code === 'Numpad0') return go('zoomReset');
  if (k === 'Tab' && S.docs.length > 1) {
    e.preventDefault();
    const i = S.docs.indexOf(S.active);
    activate(S.docs[(i + (e.shiftKey ? -1 : 1) + S.docs.length) % S.docs.length]);
  }
});

addEventListener('beforeunload', (e) => {
  if (S.docs.some((d) => d.ed && d.ed.isDirty())) {
    e.preventDefault();
    e.returnValue = '';
  }
});

/* ==================================================================== modals */

const modalStack = [];

function openModal({ title, icon: ic, body, buttons = [], wide, onCancel }) {
  const back = h('div', { class: 'modal-backdrop' });
  const foot = h('div', { class: 'modal-foot' });
  const box = h('div', { class: 'modal' + (wide ? ' wide' : '') },
    h('div', { class: 'modal-head', html: (ic ? icon(ic) : '') + `<span>${escHtml(title)}</span>` }),
    h('div', { class: 'modal-body' }, body),
    foot);
  back.append(box);
  const st = {
    back,
    onCancel,
    close() {
      back.remove();
      const i = modalStack.indexOf(st);
      if (i >= 0) modalStack.splice(i, 1);
    },
  };
  for (const b of buttons) {
    const btn = h('button', { class: 'btn' + (b.primary ? ' primary' : '') + (b.danger ? ' danger' : ''), text: b.label, onclick: () => b.onclick(st) });
    if (b.primary || b.danger) st.primary = btn;
    foot.append(btn);
  }
  if (!buttons.length) foot.remove();
  back.addEventListener('mousedown', (e) => { if (e.target === back) cancelTopModal(); });
  document.body.append(back);
  modalStack.push(st);
  return st;
}

function cancelTopModal() {
  const st = modalStack[modalStack.length - 1];
  if (!st) return;
  if (st.onCancel) st.onCancel(st); else st.close();
}

function promptBox(title, { label, value = '', placeholder = '', hint, ok = 'OK', select } = {}) {
  return new Promise((resolve) => {
    const input = h('input', { placeholder, spellcheck: 'false' });
    input.value = value;
    const done = (st, v) => { st.close(); resolve(v); };
    const st = openModal({
      title,
      body: h('div', { class: 'form-row' }, label ? h('label', { text: label }) : null, h('div', { class: 'field' }, input), hint ? h('div', { class: 'hint', text: hint }) : null),
      buttons: [{ label: 'Annuler', onclick: (s) => done(s, null) }, { label: ok, primary: true, onclick: (s) => done(s, input.value.trim()) }],
      onCancel: (s) => done(s, null),
    });
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); done(st, input.value.trim()); } });
    setTimeout(() => {
      input.focus();
      if (select) input.setSelectionRange(select[0], select[1]); else input.select();
    }, 0);
  });
}

function confirmBox(title, text, ok = 'Confirmer', danger = false) {
  return new Promise((resolve) => {
    const st = openModal({
      title,
      icon: danger ? 'warning' : null,
      body: h('p', { text }),
      buttons: [{ label: 'Annuler', onclick: (s) => { s.close(); resolve(false); } }, { label: ok, primary: !danger, danger, onclick: (s) => { s.close(); resolve(true); } }],
      onCancel: (s) => { s.close(); resolve(false); },
    });
    setTimeout(() => st.primary && st.primary.focus(), 0);
  });
}

function choiceBox(title, text, labels) {
  return new Promise((resolve) => {
    const st = openModal({
      title,
      icon: 'warning',
      body: h('p', { text }),
      buttons: labels.map((l, i) => ({ label: l, primary: i === 0, onclick: (s) => { s.close(); resolve(i); } })),
      onCancel: (s) => { s.close(); resolve(null); },
    });
    setTimeout(() => st.primary && st.primary.focus(), 0);
  });
}

/* ==================================================================== palette */

let paletteState = null;

function closePalette() {
  if (!paletteState) return;
  const st = paletteState;
  paletteState = null;
  st.back.remove();
  st.box.remove();
  if (st.prevFocus && document.contains(st.prevFocus)) st.prevFocus.focus({ preventScroll: true });
}

function openPalette(mode) {
  closePalette();
  const input = h('input', { spellcheck: 'false' });
  const list = h('div', { class: 'pal-list' });
  const back = h('div', { class: 'palette-backdrop', onmousedown: closePalette });
  const box = h('div', { class: 'palette' }, h('div', { class: 'field' }, input), list);
  document.body.append(back, box);
  const st = paletteState = { back, box, input, list, items: [], index: 0, mode, prevFocus: document.activeElement };
  input.value = mode === 'commands' ? '> ' : mode === 'line' ? ':' : '';
  input.placeholder = mode === 'recent' ? 'Choisir un dossier récent' : 'Rechercher un fichier — « > » pour les commandes, « : » pour aller à une ligne';

  const render = () => {
    list.innerHTML = '';
    if (!st.items.length) { list.append(h('div', { class: 'pal-empty', text: st.empty || 'Aucun résultat' })); return; }
    st.items.forEach((it, i) => {
      const row = h('div', { class: 'pal-item' + (i === st.index ? ' active' : '') },
        it.icon || null,
        h('span', { class: 'label', html: it.labelHtml || escHtml(it.label) }),
        it.detail ? h('span', { class: 'detail', text: it.detail }) : null,
        it.key ? h('span', { class: 'shortcut', text: it.key }) : null);
      row.addEventListener('mousedown', (e) => { e.preventDefault(); st.index = i; accept(); });
      row.addEventListener('mousemove', () => { if (st.index !== i) { st.index = i; for (const [j, el] of [...list.children].entries()) el.classList.toggle('active', j === i); } });
      list.append(row);
    });
    const a = list.children[st.index];
    if (a) a.scrollIntoView({ block: 'nearest' });
  };
  const accept = () => {
    const it = st.items[st.index];
    if (!it) return;
    closePalette();
    it.run();
  };
  const update = () => {
    const v = input.value;
    st.index = 0;
    st.empty = null;
    if (st.mode === 'recent') {
      const q = v.trim().toLowerCase();
      st.items = settings.recent.filter((p) => p.toLowerCase().includes(q)).map((p) => ({
        label: p.split(/[\\/]/).pop(), detail: p, icon: fileIconEl('', true), run: () => openFolder(p),
      }));
      st.empty = 'Aucun dossier récent';
    } else if (v.startsWith('>')) {
      const q = v.slice(1).trim().toLowerCase();
      const out = [];
      for (const id in CMD) {
        const c = CMD[id];
        if (c.enabled && !c.enabled()) continue;
        const label = typeof c.label === 'function' ? c.label() : c.label;
        let score = 0, hl = [];
        if (q) {
          const m = fuzzy(label, q);
          const at = label.toLowerCase().indexOf(q);
          if (m) { score = m.score; hl = m.hl; } else if (at >= 0) { score = 20; hl = [[at, at + q.length]]; } else continue;
        }
        out.push({ score, label, labelHtml: highlightLabel(label, hl), key: c.key, run: () => run(id) });
      }
      if (q) out.sort((a, b) => b.score - a.score);
      st.items = out.slice(0, 60);
    } else if (v.startsWith(':')) {
      const n = parseInt(v.slice(1), 10);
      const ed = S.active && S.active.ed;
      if (!ed) { st.items = []; st.empty = 'Aucun éditeur ouvert'; } else {
        const info = ed.cursorInfo();
        st.items = [{
          label: Number.isFinite(n) ? `Aller à la ligne ${n}` : `Tapez un numéro de ligne (1 à ${ed.lines.length})`,
          detail: `ligne actuelle : ${info.line}`,
          run: () => { if (Number.isFinite(n)) ed.goTo(clamp(n, 1, ed.lines.length) - 1, 0); },
        }];
      }
    } else {
      const q = v.trim().toLowerCase();
      if (!S.root) { st.items = []; st.empty = 'Aucun dossier ouvert'; render(); return; }
      let files;
      if (!q) {
        const open = S.docs.map((d) => d.path);
        files = [...open, ...S.files.filter((f) => !open.includes(f))].slice(0, 60).map((f) => ({ f, hl: [] }));
      } else {
        files = [];
        for (const f of S.files) {
          const name = baseName(f);
          const m = fuzzy(name, q);
          if (m) files.push({ f, hl: m.hl, score: m.score + 10 });
          else if (f.toLowerCase().includes(q)) files.push({ f, hl: [], score: 5 });
        }
        files.sort((a, b) => b.score - a.score || a.f.length - b.f.length);
        files = files.slice(0, 60);
      }
      st.items = files.map(({ f, hl }) => ({
        label: baseName(f), labelHtml: highlightLabel(baseName(f), hl), detail: dirName(f), icon: fileIconEl(baseName(f)), run: () => openFile(f),
      }));
      st.empty = 'Aucun fichier correspondant';
    }
    render();
  };
  input.addEventListener('input', update);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); if (st.items.length) { st.index = (st.index + 1) % st.items.length; render(); } }
    else if (e.key === 'ArrowUp') { e.preventDefault(); if (st.items.length) { st.index = (st.index - 1 + st.items.length) % st.items.length; render(); } }
    else if (e.key === 'Enter') { e.preventDefault(); accept(); }
    else if (e.key === 'Escape') { e.preventDefault(); closePalette(); }
  });
  update();
  input.focus();
  input.setSelectionRange(input.value.length, input.value.length);
}

/* ==================================================================== workspace */

function applyState(st) {
  const rootChanged = st.root !== S.root;
  S.root = st.root;
  S.rootName = st.rootName;
  S.nxc = st.nxc;
  S.port = st.port;
  S.sep = st.sep || '\\';
  S.tree = st.tree || [];
  S.treeJson = JSON.stringify(S.tree);
  S.files = [];
  const walk = (items) => { for (const it of items) { if (it.dir) walk(it.children || []); else S.files.push(it.path); } };
  walk(S.tree);
  if (rootChanged) {
    S.expanded = new Set(rootState(S.root).expanded || []);
    S.checks.clear();
    S.fileDiags.clear();
    S.selected = null;
    S.inline = null;
    if (S.live.on) stopLive();
  }
  renderTree();
  updateTitle();
  updateStatus();
  renderWelcome();
  if (S.sidebarView === 'web') renderWebView();
}

async function refreshTree() {
  try {
    const st = await api.get('state');
    if (JSON.stringify(st.tree) !== S.treeJson || st.root !== S.root || st.nxc !== S.nxc) applyState(st);
  } catch (e) { /* server busy */ }
}

async function openFolder(path) {
  if (!(await closeAllDocs())) return;
  let st;
  try { st = await api.post('open-folder', { path }); } catch (e) { toast(e.message, 'error'); return; }
  applyState(st);
  rememberRoot();
  await restoreSession();
}

function rememberRoot() {
  if (!S.root) return;
  settings.recent = [S.root, ...settings.recent.filter((r) => r.toLowerCase() !== S.root.toLowerCase())].slice(0, 10);
  settings.lastRoot = S.root;
  saveSettings();
}

async function closeFolder() {
  if (!(await closeAllDocs())) return;
  stopLive();
  const st = await api.post('close-folder');
  settings.lastRoot = null;
  saveSettings();
  applyState(st);
}

function saveSession() {
  if (!S.root) return;
  rootState(S.root).session = { open: S.docs.map((d) => d.path), active: S.active ? S.active.path : null };
  persist();
}

async function restoreSession() {
  const ses = rootState(S.root).session;
  if (!ses) {
    // A fresh folder: open its main file, if any.
    const main = S.files.find((f) => /(^|\/)main\.nx$/.test(f)) || S.files.find((f) => f === 'index.html');
    if (main) await openFile(main);
    return;
  }
  for (const p of ses.open) if (S.files.includes(p)) await openFile(p, { background: true });
  const act = S.docs.find((d) => d.path === ses.active) || S.docs[0];
  if (act) activate(act);
}

function saveExpanded() { if (S.root) { rootState(S.root).expanded = [...S.expanded]; persist(); } }

/* ==================================================================== explorer */

function renderTree() {
  const view = $('.sb-view[data-view="explorer"]');
  if (!view) return;
  if (!S.root) {
    view.innerHTML = '';
    view.append(
      h('div', { class: 'sb-header' }, h('span', { text: 'Explorateur' })),
      h('div', { class: 'sb-empty' },
        h('p', { text: "Aucun dossier n'est ouvert." }),
        h('button', { class: 'btn primary block', html: icon('folderOpen') + 'Ouvrir un dossier', onclick: () => run('openFolder') }),
        h('button', { class: 'btn block', html: icon('sparkle') + 'Nouveau projet', onclick: () => run('newProject') }),
        h('p', { style: { marginTop: '14px', fontSize: '12px' }, text: 'Un projet NEXUL contient un fichier main.nx ; un site web, un fichier index.html.' })),
    );
    return;
  }
  let body = $('#tree');
  if (!body) {
    view.innerHTML = '';
    const actions = h('div', { class: 'sb-actions' },
      h('button', { class: 'icon-btn', title: 'Nouveau fichier', html: icon('newFile'), onclick: () => startInlineNew('file') }),
      h('button', { class: 'icon-btn', title: 'Nouveau dossier', html: icon('newFolder'), onclick: () => startInlineNew('dir') }),
      h('button', { class: 'icon-btn', title: 'Actualiser', html: icon('refresh'), onclick: () => refreshTree() }),
      h('button', { class: 'icon-btn', title: 'Tout réduire', html: icon('collapse'), onclick: () => { S.expanded.clear(); saveExpanded(); renderTree(); } }));
    body = h('div', { class: 'sb-body', id: 'tree', tabindex: '0' });
    view.append(
      h('div', { class: 'sb-header' }, h('span', { text: 'Explorateur' }), actions),
      h('div', { class: 'sb-section-title', id: 'tree-title', title: '', onclick: () => { S.selected = null; renderTree(); } }),
      body);
    body.addEventListener('click', onTreeClick);
    body.addEventListener('contextmenu', onTreeContext);
    body.addEventListener('keydown', onTreeKey);
  }
  const title = $('#tree-title');
  title.innerHTML = icon('chevronDown') + `<span>${escHtml(S.rootName || '')}</span>`;
  title.title = S.root;
  const scroll = body.scrollTop;
  body.innerHTML = '';
  const frag = document.createDocumentFragment();
  const inline = S.inline;
  const inlineRow = (depth) => {
    const input = h('input', { class: 'tree-input', spellcheck: 'false' });
    if (inline.value !== undefined) input.value = inline.value;
    const row = h('div', { class: 'tree-row', style: { paddingLeft: 8 + depth * 14 + 'px' } },
      h('span', { class: 'twisty' }), inline.kind === 'dir' ? fileIconEl('', true) : iconEl('file'), input);
    setupInlineInput(input, inline);
    return row;
  };
  const walk = (items, depth, parent) => {
    if (inline && inline.kind !== 'rename' && inline.parent === parent) frag.append(inlineRow(depth));
    for (const it of items) {
      const open = it.dir && S.expanded.has(it.path);
      const row = h('div', {
        class: 'tree-row' + (S.selected === it.path ? ' selected' : ''),
        dataset: { path: it.path, dir: it.dir ? '1' : '' },
        style: { paddingLeft: 8 + depth * 14 + 'px' },
        title: it.path,
      });
      for (let k = 0; k < depth; k++) row.append(h('div', { class: 'tree-guide', style: { left: 15 + k * 14 + 'px' } }));
      row.append(it.dir ? h('span', { class: 'twisty' + (open ? ' open' : ''), html: icon('chevronRight') }) : h('span', { class: 'twisty' }));
      row.append(fileIconEl(it.name, it.dir, open));
      if (inline && inline.kind === 'rename' && inline.path === it.path) {
        const input = h('input', { class: 'tree-input', spellcheck: 'false' });
        input.value = inline.value !== undefined ? inline.value : it.name;
        setupInlineInput(input, inline);
        row.append(input);
      } else {
        const doc = S.docs.find((d) => d.path === it.path);
        row.append(h('span', { class: 'name' + (doc && doc.ed && doc.ed.isDirty() ? ' modified' : ''), text: it.name }));
        const c = diagCounts(it.path, it.dir);
        if (c.err) row.append(h('span', { class: 'diag-count err', text: c.err }));
        else if (c.warn) row.append(h('span', { class: 'diag-count warn', text: c.warn }));
      }
      frag.append(row);
      if (open) walk(it.children || [], depth + 1, it.path);
    }
  };
  walk(S.tree, 0, '');
  if (!S.tree.length && !inline) frag.append(h('div', { class: 'sb-empty' }, h('p', { text: 'Ce dossier est vide.' }),
    h('button', { class: 'btn block', html: icon('newFile') + 'Nouveau fichier', onclick: () => startInlineNew('file') })));
  body.append(frag);
  body.scrollTop = scroll;
  const input = body.querySelector('.tree-input');
  if (input) {
    input.focus();
    if (!inline.sel) {
      const dot = input.value.lastIndexOf('.');
      inline.sel = [0, inline.kind === 'rename' && dot > 0 ? dot : input.value.length];
      input.scrollIntoView({ block: 'nearest' });
    }
    input.setSelectionRange(inline.sel[0], inline.sel[1]);
  }
}

function diagCounts(path, dir) {
  let err = 0, warn = 0;
  for (const [p, list] of S.fileDiags) {
    if (dir ? !p.startsWith(path + '/') : p !== path) continue;
    for (const d of list) if (d.level === 'error') err++; else warn++;
  }
  return { err, warn };
}

function onTreeClick(e) {
  const row = e.target.closest('.tree-row');
  if (!row || !row.dataset.path || e.target.classList.contains('tree-input')) return;
  const p = row.dataset.path;
  S.selected = p;
  if (row.dataset.dir) {
    if (S.expanded.has(p)) S.expanded.delete(p); else S.expanded.add(p);
    saveExpanded();
    renderTree();
  } else {
    // A click keeps the focus in the explorer (F2, Suppr…); a double-click goes to the editor.
    openFile(p, { keepFocus: e.detail < 2 });
  }
}

function onTreeKey(e) {
  if (e.target.classList.contains('tree-input')) return;
  const rows = $$('.tree-row[data-path]', $('#tree'));
  const i = rows.findIndex((r) => r.dataset.path === S.selected);
  const sel = (r) => { if (r) { S.selected = r.dataset.path; renderTree(); } };
  if (e.key === 'ArrowDown') { e.preventDefault(); sel(rows[Math.min(rows.length - 1, i + 1)]); }
  else if (e.key === 'ArrowUp') { e.preventDefault(); sel(rows[Math.max(0, i - 1)]); }
  else if (e.key === 'Enter' && i >= 0) { e.preventDefault(); rows[i].click(); }
  else if (e.key === 'ArrowRight' && i >= 0 && rows[i].dataset.dir && !S.expanded.has(S.selected)) { S.expanded.add(S.selected); saveExpanded(); renderTree(); }
  else if (e.key === 'ArrowLeft' && i >= 0) {
    if (rows[i].dataset.dir && S.expanded.has(S.selected)) { S.expanded.delete(S.selected); saveExpanded(); renderTree(); }
    else if (dirName(S.selected)) { S.selected = dirName(S.selected); renderTree(); }
  }
}

function onTreeContext(e) {
  e.preventDefault();
  const row = e.target.closest('.tree-row');
  const p = row && row.dataset.path ? row.dataset.path : '';
  const isDir = !p || !!row.dataset.dir;
  if (p) { S.selected = p; renderTree(); }
  const parent = isDir ? p : dirName(p);
  const items = [
    { label: 'Nouveau fichier', run: () => startInlineNew('file', parent) },
    { label: 'Nouveau dossier', run: () => startInlineNew('dir', parent) },
  ];
  if (p && !isDir) {
    items.unshift({ label: 'Ouvrir', run: () => openFile(p) }, '-');
    if (extOf(p) === 'nx') items.push('-', { label: 'Exécuter', key: 'F5', run: () => openFile(p).then(() => runProgram('run')) }, { label: 'Lancer les tests', run: () => openFile(p).then(() => runProgram('test')) });
    if (extOf(p) === 'html') items.push('-', { label: "Ouvrir l'aperçu en direct", run: () => startLive(p) });
  }
  items.push('-', { label: 'Copier le chemin', run: () => copyText(absPath(p)) }, { label: 'Copier le chemin relatif', run: () => copyText(p || '.') },
    { label: "Afficher dans l'Explorateur Windows", run: () => api.post('reveal?' + qs({ path: p })) });
  if (p) items.push('-', { label: 'Renommer…', key: 'F2', run: () => startRename(p) }, { label: 'Supprimer', key: 'Suppr', run: () => deletePath(p) });
  showMenu(items, e.clientX, e.clientY);
}

function copyText(t) {
  if (navigator.clipboard) navigator.clipboard.writeText(t).then(() => toast('Copié dans le presse-papiers.', 'success', 1800)).catch(() => {});
}

function startInlineNew(kind, parent) {
  if (!S.root) { toast("Ouvrez d'abord un dossier ou créez un projet.", 'error'); return; }
  if (parent === undefined) {
    const sel = S.selected;
    const isDir = sel && !S.files.includes(sel);
    parent = sel ? (isDir ? sel : dirName(sel)) : (S.active ? dirName(S.active.path) : '');
  }
  let p = parent;
  while (p) { S.expanded.add(p); p = dirName(p); }
  S.inline = { kind, parent };
  showView('explorer');
  renderTree();
}

function startRename(path) {
  S.inline = { kind: 'rename', path };
  showView('explorer');
  renderTree();
}

function setupInlineInput(input, inline) {
  let done = false;
  const remember = () => { inline.value = input.value; inline.sel = [input.selectionStart, input.selectionEnd]; };
  for (const ev of ['input', 'keyup', 'mouseup']) input.addEventListener(ev, remember);
  const finish = async (commit) => {
    if (done || S.inline !== inline) return;
    done = true;
    const name = input.value.trim();
    S.inline = null;
    if (!commit || !name || (inline.kind === 'rename' && name === baseName(inline.path))) { renderTree(); return; }
    if (/[\\:*?"<>|]/.test(name) || name.split('/').some((s) => s === '..')) { toast('Nom invalide : les caractères \\ : * ? " < > | sont interdits.', 'error'); renderTree(); return; }
    try {
      if (inline.kind === 'rename') {
        const to = joinRel(dirName(inline.path), name);
        const st = await api.post('rename', { from: inline.path, to });
        renamed(inline.path, to);
        applyState(st);
        S.selected = to;
      } else {
        const path = normRel(joinRel(inline.parent, name));
        const st = await api.post('new?' + qs({ path, kind: inline.kind }), inline.kind === 'file' ? starterContent(path) : '');
        applyState(st);
        S.selected = path;
        if (inline.kind === 'file') await openFile(path); else { S.expanded.add(path); saveExpanded(); }
      }
    } catch (e) { toast(e.message, 'error'); }
    renderTree();
  };
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); finish(true); }
    else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); finish(false); }
  });
  input.addEventListener('blur', () => { if (input.isConnected) finish(true); });
  input.addEventListener('click', (e) => e.stopPropagation());
}

function starterContent(path) {
  const ext = extOf(path);
  if (ext === 'nx' && /(^|\/)main\.nx$/.test(path)) return 'fn main() {\n    println("Bonjour !")\n}\n';
  return '';
}

function renamed(from, to) {
  for (const d of S.docs) {
    if (d.path === from || d.path.startsWith(from + '/')) {
      const np = to + d.path.slice(from.length);
      d.path = np;
      d.name = baseName(np);
      if (d.ed) d.ed.setLang(langOf(np), np);
    }
  }
  for (const k of [...S.checks.keys()]) if (k === from || k.startsWith(from + '/')) S.checks.delete(k);
  refreshDiagnostics();
  if (S.live.path && (S.live.path === from || S.live.path.startsWith(from + '/'))) setPreview(to + S.live.path.slice(from.length), true);
  saveSession();
}

async function deletePath(path) {
  const ok = await confirmBox('Supprimer', `Supprimer « ${baseName(path)} » ? Il sera placé dans la Corbeille.`, 'Supprimer', true);
  if (!ok) return;
  try {
    const st = await api.post('delete?' + qs({ path }));
    for (const d of [...S.docs]) if (d.path === path || d.path.startsWith(path + '/')) await closeDoc(d, true);
    for (const k of [...S.checks.keys()]) if (k === path || k.startsWith(path + '/')) S.checks.delete(k);
    S.selected = null;
    applyState(st);
    refreshDiagnostics();
  } catch (e) { toast(e.message, 'error'); }
}

/* ==================================================================== documents */

const editorHooks = {
  change(ed) {
    const doc = S.docs.find((d) => d.ed === ed);
    if (!doc) return;
    updateDirty(doc);
    if (settings.autoSave) {
      clearTimeout(doc.saveTimer);
      const fast = S.live.on && /^(html|css|js)$/.test(ed.lang);
      doc.saveTimer = setTimeout(() => { if (ed.isDirty()) saveDoc(doc); }, fast ? 300 : 900);
    }
  },
  cursor(ed) { if (S.active && S.active.ed === ed) updateCursorStatus(); },
  explain(code) { explain(code); },
  contextmenu(e, ed) { editorContextMenu(e, ed); },
};

function imageView(path) {
  return h('div', { class: 'welcome', style: { alignItems: 'center', justifyContent: 'center', display: 'flex' } },
    h('img', {
      src: livePath(path) + '?v=' + Date.now(),
      alt: baseName(path),
      style: { maxWidth: '88%', maxHeight: '88%', boxShadow: 'var(--shadow)', borderRadius: '6px', background: 'repeating-conic-gradient(#d8d8e0 0 25%, #fff 0 50%) 0 0/16px 16px' },
    }));
}

async function openFile(path, opts = {}) {
  let doc = S.docs.find((d) => d.path === path);
  if (!doc) {
    if (IMAGE_EXT.has(extOf(path))) {
      doc = { path, name: baseName(path), el: imageView(path) };
    } else {
      let text;
      try { text = await api.get('file?' + qs({ path })); } catch (e) { toast(`${baseName(path)} : ${e.message}`, 'error'); return null; }
      doc = S.docs.find((d) => d.path === path);
      if (!doc) {
        const ed = new Editor({ path, lang: langOf(path), text }, editorHooks);
        doc = { path, name: baseName(path), ed, el: ed.el };
        ed.setDiagnostics(S.fileDiags.get(path) || []);
      }
    }
    if (!S.docs.includes(doc)) {
      const i = S.active ? S.docs.indexOf(S.active) + 1 : S.docs.length;
      S.docs.splice(i, 0, doc);
      if (!doc.ed) doc.el.style.display = 'none';
      $('#editor-container').append(doc.el);
      if (extOf(path) === 'nx' && !S.checks.has(path)) scheduleCheck(path, 60);
    }
  }
  if (!opts.background) activate(doc, opts.keepFocus);
  if (opts.line !== undefined && doc.ed) {
    const ed = doc.ed;
    const l = clamp(opts.line, 0, ed.lines.length - 1);
    const c = opts.charCol !== undefined ? cpIndex(ed.lines[l], opts.charCol) : opts.col || 0;
    const len = opts.len || 0;
    ed.goTo(l, c, len ? l : undefined, len ? c + len : undefined);
  }
  saveSession();
  return doc;
}

function activate(doc, keepFocus) {
  if (S.active && S.active !== doc) {
    if (S.active.ed) S.active.ed.hide(); else S.active.el.style.display = 'none';
  }
  S.active = doc || null;
  if (doc) {
    if (doc.ed) {
      doc.ed.show();
      if (!keepFocus) setTimeout(() => { if (S.active === doc && !modalStack.length && !paletteState) doc.ed.focus(); }, 0);
    }
    else doc.el.style.display = 'flex';
    S.selected = doc.path;
    let p = dirName(doc.path);
    let changed = false;
    while (p) { if (!S.expanded.has(p)) { S.expanded.add(p); changed = true; } p = dirName(p); }
    if (changed) saveExpanded();
    if (S.live.on && extOf(doc.path) === 'html') setPreview(doc.path);
  }
  renderTabs();
  renderBreadcrumbs();
  renderTree();
  renderWelcome();
  updateTitle();
  updateStatus();
  saveSession();
  const row = $('#tree .tree-row.selected');
  if (row) row.scrollIntoView({ block: 'nearest' });
}

async function closeDoc(doc, force) {
  if (!force && doc.ed && doc.ed.isDirty()) {
    if (settings.autoSave) {
      if (!(await saveDoc(doc))) return false;
    } else {
      const r = await choiceBox(`Enregistrer ${doc.name} ?`, 'Vos modifications seront perdues si vous ne les enregistrez pas.', ['Enregistrer', 'Ne pas enregistrer', 'Annuler']);
      if (r === null || r === 2) return false;
      if (r === 0 && !(await saveDoc(doc))) return false;
    }
  }
  const i = S.docs.indexOf(doc);
  if (i < 0) return true;
  clearTimeout(doc.saveTimer);
  S.docs.splice(i, 1);
  if (doc.ed) doc.ed.dispose(); else doc.el.remove();
  if (S.active === doc) {
    S.active = null;
    activate(S.docs[Math.min(i, S.docs.length - 1)] || null);
  } else renderTabs();
  saveSession();
  return true;
}

async function closeAllDocs() {
  for (const d of [...S.docs]) if (!(await closeDoc(d))) return false;
  return true;
}

async function saveDoc(doc, manual) {
  if (!doc || !doc.ed) return true;
  const ed = doc.ed;
  clearTimeout(doc.saveTimer);
  const value = ed.value;
  if (!manual && value === ed.savedValue) return true;
  try {
    await api.post('file?' + qs({ path: doc.path }), ed.getText());
  } catch (e) {
    toast(`Enregistrement de ${doc.name} impossible : ${e.message}`, 'error');
    return false;
  }
  ed.savedValue = value;
  updateDirty(doc);
  if (extOf(doc.path) === 'nx' && settings.checkOnSave) scheduleCheck(doc.path, 120);
  return true;
}

async function saveAll() {
  let ok = true;
  for (const d of S.docs) if (d.ed && d.ed.isDirty()) ok = (await saveDoc(d)) && ok;
  return ok;
}

function updateDirty(doc) {
  const dirty = !!(doc.ed && doc.ed.isDirty());
  if (doc.wasDirty === dirty) return;
  doc.wasDirty = dirty;
  const tab = $$('#tabs .tab').find((t) => t.dataset.path === doc.path);
  if (tab) tab.classList.toggle('dirty', dirty);
  const row = $$('#tree .tree-row').find((r) => r.dataset.path === doc.path);
  if (row) { const n = row.querySelector('.name'); if (n) n.classList.toggle('modified', dirty); }
}

function renderTabs() {
  const box = $('#tabs');
  box.innerHTML = '';
  const names = new Map();
  for (const d of S.docs) names.set(d.name, (names.get(d.name) || 0) + 1);
  for (const d of S.docs) {
    const diags = S.fileDiags.get(d.path) || [];
    const err = diags.some((x) => x.level === 'error'), warn = !err && diags.length > 0;
    const label = names.get(d.name) > 1 && dirName(d.path) ? `${d.name} · ${baseName(dirName(d.path))}` : d.name;
    const tab = h('div', {
      class: 'tab' + (d === S.active ? ' active' : '') + (d.ed && d.ed.isDirty() ? ' dirty' : ''),
      title: absPath(d.path), dataset: { path: d.path }, role: 'tab',
    },
    fileIconEl(d.name),
    h('span', { class: 'tab-name' + (err ? ' err' : warn ? ' warn' : ''), text: label }),
    h('span', { class: 'tab-close', title: 'Fermer (Ctrl+W)', html: icon('close') }));
    d.wasDirty = !!(d.ed && d.ed.isDirty());
    tab.addEventListener('mousedown', (e) => { if (e.button === 1) { e.preventDefault(); closeDoc(d); } });
    tab.addEventListener('click', (e) => { if (e.target.closest('.tab-close')) closeDoc(d); else activate(d); });
    tab.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      showMenu([
        { label: 'Fermer', key: 'Ctrl+W', run: () => closeDoc(d) },
        { label: 'Fermer les autres', run: async () => { for (const o of [...S.docs]) if (o !== d && !(await closeDoc(o))) return; } },
        { label: 'Tout fermer', run: () => closeAllDocs() },
        '-',
        { label: 'Copier le chemin', run: () => copyText(absPath(d.path)) },
        { label: "Afficher dans l'explorateur", run: () => { S.selected = d.path; showView('explorer'); renderTree(); } },
      ], e.clientX, e.clientY);
    });
    box.append(tab);
  }
  const act = box.querySelector('.tab.active');
  if (act) act.scrollIntoView({ inline: 'nearest', block: 'nearest' });
}

function renderBreadcrumbs() {
  const bc = $('#breadcrumbs');
  bc.innerHTML = '';
  if (!S.active) { bc.style.display = 'none'; return; }
  bc.style.display = '';
  const parts = [S.rootName || '', ...S.active.path.split('/')];
  parts.forEach((p, i) => {
    if (i) bc.append(h('span', { class: 'crumb-sep', html: icon('chevronRight') }));
    const last = i === parts.length - 1;
    bc.append(h('span', { class: 'crumb', style: { display: 'flex', alignItems: 'center', gap: '5px' } }, last ? fileIconEl(p) : null, p));
  });
}

function renderWelcome() {
  let w = $('#welcome');
  if (S.docs.length) { if (w) w.remove(); return; }
  if (!w) { w = h('div', { class: 'welcome', id: 'welcome' }); $('#editor-container').append(w); }
  const recent = settings.recent.slice(0, 6);
  const ex = examplesDir();
  w.innerHTML = `
  <div class="welcome-inner">
    <div class="welcome-hero">
      <div class="welcome-logo">N</div>
      <div><h1><span>NEXUL</span> Studio</h1>
      <div class="tagline">Écrivez, exécutez et testez du NEXUL — et concevez des sites web avec un aperçu en direct.</div></div>
    </div>
    <div class="welcome-grid">
      <div>
        <h2>Démarrer</h2>
        <div class="w-action" data-cmd="newProject">${icon('sparkle')}<span>Nouveau projet…</span><small>NEXUL ou site web</small></div>
        <div class="w-action" data-cmd="openFolder">${icon('folderOpen')}<span>Ouvrir un dossier…</span><small>Ctrl+O</small></div>
        ${S.root ? `<div class="w-action" data-cmd="newFile">${icon('newFile')}<span>Nouveau fichier</span><small>Ctrl+N</small></div>` : ''}
        <div class="w-action" data-cmd="newSite">${icon('web')}<span>Créer un site web</span><small>HTML · CSS · JS</small></div>
        ${ex ? `<div class="w-action" data-cmd="openExamples">${icon('book')}<span>Ouvrir les exemples NEXUL</span><small>${escHtml(ex)}</small></div>` : ''}
      </div>
      <div>
        <h2>Récents</h2>
        ${recent.length ? recent.map((r, i) => `<div class="w-recent" data-recent="${i}"><b>${escHtml(r.split(/[\\/]/).pop())}</b><span>${escHtml(r)}</span></div>`).join('')
    : '<div style="color:var(--fg-muted)">Aucun dossier récent.</div>'}
      </div>
    </div>
    <div class="w-cards">
      <div class="w-card"><h3>${icon('play')}Exécuter</h3><p>Ouvrez un fichier <code>.nx</code> et appuyez sur <span class="kbd">F5</span> : le programme est compilé puis lancé dans le terminal.</p></div>
      <div class="w-card"><h3>${icon('live')}Aperçu en direct</h3><p>Ouvrez une page HTML puis cliquez sur <b>Aperçu</b> : chaque modification recharge la page instantanément.</p></div>
      <div class="w-card"><h3>${icon('check')}Vérification continue</h3><p>Les erreurs sont soulignées pendant que vous écrivez. Survolez-les pour comprendre et corriger.</p></div>
    </div>
  </div>`;
  w.onclick = (e) => {
    const a = e.target.closest('[data-cmd]');
    if (a) return run(a.dataset.cmd);
    const r = e.target.closest('[data-recent]');
    if (r) openFolder(settings.recent[+r.dataset.recent]);
  };
}

function updateTitle() {
  document.title = (S.active ? S.active.name + ' — ' : '') + (S.rootName ? S.rootName + ' — ' : '') + 'NEXUL Studio';
  $('#cc-label').textContent = S.rootName || 'NEXUL Studio';
}

function editorContextMenu(e, ed) {
  const sel = ed.ta.selectionStart !== ed.ta.selectionEnd;
  const items = [
    { label: 'Couper', key: 'Ctrl+X', run: () => ed.command('cut') },
    { label: 'Copier', key: 'Ctrl+C', run: () => ed.command('copy') },
    { label: 'Coller', key: 'Ctrl+V', run: () => ed.command('paste') },
    '-',
    { label: 'Commenter / décommenter', key: 'Ctrl+/', run: () => ed.command('comment') },
    { label: 'Dupliquer la ligne', key: 'Maj+Alt+↓', run: () => ed.command('duplicate') },
    { label: 'Rechercher', key: 'Ctrl+F', run: () => ed.command('find') },
    { label: 'Remplacer', key: 'Ctrl+H', run: () => ed.command('replace') },
    '-',
    { label: 'Tout sélectionner', key: 'Ctrl+A', run: () => ed.command('selectAll') },
  ];
  if (!sel) items[0].label = 'Couper la ligne';
  if (ed.lang === 'nexul') items.push('-', { label: 'Exécuter', key: 'F5', run: () => runProgram('run') }, { label: 'Vérifier', key: 'F7', run: () => checkCommand() });
  if (ed.lang === 'html') items.push('-', { label: "Ouvrir l'aperçu en direct", key: 'Alt+L', run: () => startLive(ed.path) });
  showMenu(items, e.clientX, e.clientY);
}

/* ==================================================================== diagnostics */

function scheduleCheck(path, delay = 300) {
  if (!S.nxc) return;
  clearTimeout(S.checkTimers.get(path));
  S.checkTimers.set(path, setTimeout(() => { S.checkTimers.delete(path); checkFile(path); }, delay));
}

const MAIN_RE = /(^|\n)\s*(pub\s+)?fn\s+main\s*\(/;

async function textOf(path) {
  const d = S.docs.find((x) => x.path === path);
  if (d && d.ed) return d.ed.value;
  try { return await api.get('file?' + qs({ path })); } catch (e) { return ''; }
}

/* A module is checked through the program that imports it (imports are relative to the
 * folder of the main file). */
async function checkRoots(path) {
  if (MAIN_RE.test(await textOf(path))) return [path];
  const parts = path.split('/');
  for (let k = parts.length - 1; k >= 0; k--) {
    const dir = parts.slice(0, k).join('/');
    const modName = parts.slice(k).join('.').replace(/\.nx$/, '');
    const cands = S.files.filter((f) => f !== path && extOf(f) === 'nx' && dirName(f) === dir);
    const mains = [];
    let mentioned = false;
    const re = new RegExp('(^|\\n)\\s*import\\s+' + escRe(modName) + '\\b');
    for (const f of cands) {
      const t = await textOf(f);
      if (MAIN_RE.test(t)) mains.push(f);
      if (re.test(t)) mentioned = true;
    }
    if (mains.length && mentioned) return mains;
  }
  return [path];
}

async function checkFile(path) {
  if (!S.nxc || !S.root) return;
  const roots = await checkRoots(path);
  if (!roots.includes(path)) S.checks.delete(path);
  await Promise.all(roots.map(async (root) => {
    try {
      const r = await api.post('check?' + qs({ path: root }));
      S.checks.set(root, r.diagnostics.map((d) => ({ path: d.file ? normRel(joinRel(r.base, d.file)) : root, d })));
    } catch (e) {
      S.checks.set(root, [{ path: root, d: { level: 'error', code: '', message: e.message, line: 1, col: 1, end_line: 1, end_col: 1, labels: [], notes: [] } }]);
    }
  }));
  refreshDiagnostics();
}

function refreshDiagnostics() {
  const m = new Map();
  const seen = new Set();
  for (const list of S.checks.values()) {
    for (const x of list) {
      const key = [x.path, x.d.code, x.d.line, x.d.col, x.d.message].join('|');
      if (seen.has(key)) continue;
      seen.add(key);
      if (!m.has(x.path)) m.set(x.path, []);
      m.get(x.path).push(x.d);
    }
  }
  S.fileDiags = m;
  for (const d of S.docs) if (d.ed) d.ed.setDiagnostics(m.get(d.path) || []);
  renderProblems();
  renderTabs();
  renderTree();
  updateStatus();
}

function diagTotals() {
  let err = 0, warn = 0;
  for (const list of S.fileDiags.values()) for (const d of list) if (d.level === 'error') err++; else warn++;
  return { err, warn };
}

async function checkCommand() {
  if (!S.nxc) { toast('Compilateur nxc introuvable : indiquez son chemin dans les paramètres.', 'error'); return; }
  const path = S.active && extOf(S.active.path) === 'nx' ? S.active.path : findMainNx();
  if (!path) { toast('Ouvrez un fichier NEXUL (.nx) à vérifier.', 'error'); return; }
  await saveAll();
  await checkFile(path);
  const n = (S.fileDiags.get(path) || []).length;
  const t = diagTotals();
  if (!t.err && !t.warn) toast(`${baseName(path)} : aucun problème.`, 'success', 2500);
  else { showPanel('problems'); if (!n) toast(`${t.err} erreur(s), ${t.warn} avertissement(s).`, 'error'); }
}

function findMainNx() {
  if (S.active && extOf(S.active.path) === 'nx') return S.active.path;
  return S.files.find((f) => f === 'main.nx') || S.files.find((f) => /(^|\/)main\.nx$/.test(f)) || S.files.find((f) => extOf(f) === 'nx') || null;
}

function renderProblems() {
  const box = $('.problems');
  if (!box) return;
  box.innerHTML = '';
  const t = diagTotals();
  const badge = $('.panel-tab[data-tab="problems"] .count');
  if (badge) badge.textContent = t.err + t.warn;
  const files = [...S.fileDiags.entries()].filter(([, l]) => l.length);
  if (!files.length) {
    box.append(h('div', { class: 'prob-empty', html: icon('ok', 'ic-ok') + "<span>Aucun problème détecté dans l'espace de travail.</span>", style: { display: 'flex', gap: '8px', alignItems: 'center' } }));
    return;
  }
  files.sort((a, b) => b[1].filter((d) => d.level === 'error').length - a[1].filter((d) => d.level === 'error').length || a[0].localeCompare(b[0]));
  for (const [path, list] of files) {
    box.append(h('div', { class: 'prob-file' }, fileIconEl(baseName(path)), h('span', { text: baseName(path) }),
      h('span', { style: { color: 'var(--fg-muted)', fontWeight: '400', fontSize: '12px' }, text: dirName(path) }), h('span', { class: 'count', text: list.length })));
    const sorted = [...list].sort((a, b) => (a.level === 'error' ? 0 : 1) - (b.level === 'error' ? 0 : 1) || a.line - b.line);
    for (const d of sorted) {
      box.append(h('div', {
        class: 'prob-item',
        onclick: () => openFile(path, { line: d.line - 1, charCol: d.col - 1 }),
        title: [d.message, ...(d.notes || [])].join('\n'),
      },
      iconEl(d.level === 'error' ? 'error' : 'warning', d.level === 'error' ? 'ic-err' : 'ic-warn'),
      h('span', { class: 'msg', html: fmtMsg(d.message) + (d.code ? ` <small>${escHtml(d.code)}</small>` : '') }),
      h('span', { class: 'where', text: d.line ? `ligne ${d.line}, col. ${d.col}` : '' })));
    }
  }
}

async function explain(code) {
  let text;
  try { text = await api.get('explain?' + qs({ code })); } catch (e) { toast(e.message, 'error'); return; }
  const st = openModal({
    title: `Explication de ${code}`, icon: 'book', wide: true,
    body: h('div', { class: 'explain-text', text: text.trim() || 'Aucune explication pour ce code.' }),
    buttons: [{ label: 'Fermer', primary: true, onclick: (s) => s.close() }],
  });
  setTimeout(() => st.primary && st.primary.focus(), 0);
}

/* ==================================================================== panel: terminal & problems */

function buildPanel() {
  const panel = $('#panel');
  panel.innerHTML = `
  <div class="panel-head">
    <button class="panel-tab" data-tab="terminal">${icon('terminal')}Terminal</button>
    <button class="panel-tab" data-tab="problems">Problèmes <span class="count">0</span></button>
    <div class="panel-spacer"></div>
    <span id="run-state" style="color:var(--fg-muted);font-size:12px;margin-right:6px"></span>
    <button class="icon-btn" id="term-stop" title="Arrêter (Maj+F5)" style="display:none;color:var(--error)">${icon('stop')}</button>
    <button class="icon-btn" id="term-clear" title="Effacer le terminal">${icon('trash')}</button>
    <button class="icon-btn" id="panel-close" title="Fermer le panneau">${icon('close')}</button>
  </div>
  <div class="panel-body">
    <div class="panel-view" data-view="terminal">
      <div class="term-out"></div>
      <div class="term-input hidden"><span>›</span><input placeholder="Entrée du programme — appuyez sur Entrée pour l'envoyer" spellcheck="false"></div>
    </div>
    <div class="panel-view" data-view="problems"><div class="problems"></div></div>
  </div>`;
  for (const b of $$('.panel-tab', panel)) b.onclick = () => showPanel(b.dataset.tab);
  $('#term-clear').onclick = () => termClear();
  $('#term-stop').onclick = () => stopRun();
  $('#panel-close').onclick = () => hidePanel();
  const out = $('.term-out');
  out.addEventListener('click', (e) => {
    const a = e.target.closest('a[data-loc]');
    if (a) {
      const [p, l, c] = a.dataset.loc.split('|');
      openFile(p, { line: +l - 1, charCol: Math.max(0, +c - 1) });
    }
    const r = e.target.closest('a[data-reveal]');
    if (r) api.post('reveal?' + qs({ path: r.dataset.reveal }));
  });
  const input = $('.term-input input');
  input.addEventListener('keydown', async (e) => {
    if (e.key !== 'Enter' || !S.run) return;
    e.preventDefault();
    const text = input.value;
    input.value = '';
    termWrite(`\x1b[90m${text}\x1b[0m\n`);
    try { await api.post('run/input?' + qs({ id: S.run.id }), text + '\n'); } catch (err) { /* finished */ }
  });
  S.panelTab = 'terminal';
  showPanel('terminal', true);
  termWelcome();
  renderProblems();
}

const panelOpen = () => !$('#panel').classList.contains('hidden');

function showPanel(tab, keepHidden) {
  S.panelTab = tab;
  if (!keepHidden) {
    $('#panel').classList.remove('hidden');
    $('#panel-resizer').classList.remove('hidden');
    settings.panelOpen = true;
    saveSettings();
  }
  for (const b of $$('.panel-tab')) b.classList.toggle('active', b.dataset.tab === tab);
  for (const v of $$('.panel-view')) v.classList.toggle('active', v.dataset.view === tab);
  if (tab === 'terminal' && S.run && !keepHidden) setTimeout(() => $('.term-input input').focus(), 0);
}

function hidePanel() {
  $('#panel').classList.add('hidden');
  $('#panel-resizer').classList.add('hidden');
  settings.panelOpen = false;
  saveSettings();
  if (S.active && S.active.ed) S.active.ed.focus();
}

function togglePanel(tab) {
  if (panelOpen() && S.panelTab === tab) hidePanel(); else showPanel(tab);
}

const term = { color: '', bold: false };

function termClear() {
  $('.term-out').innerHTML = '';
  term.color = '';
  term.bold = false;
}

function termWelcome() {
  termWrite('\x1b[90mAppuyez sur F5 pour exécuter le fichier NEXUL ouvert, F6 pour lancer ses tests.\x1b[0m\n');
}

const ANSI_CLASS = { 30: 't-dim', 90: 't-dim', 31: 'ansi-31', 91: 'ansi-31', 32: 'ansi-32', 92: 'ansi-32', 33: 'ansi-33', 93: 'ansi-33', 34: 'ansi-34', 94: 'ansi-34', 35: 'ansi-34', 95: 'ansi-34', 36: 'ansi-36', 96: 'ansi-36', 37: '', 97: '' };

function termWrite(text, runInfo) {
  const el = $('.term-out');
  const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 6;
  text = text.replace(/\r(?!\n)/g, '').replace(/\x1b\[[\d;?]*[A-Za-ln-z]/g, '');
  const re = /\x1b\[([\d;]*)m/g;
  let html = '', last = 0, m;
  const flush = (s) => {
    if (!s) return;
    let x = escHtml(s);
    if (runInfo) {
      x = x.replace(/([\w.\/\\-]+\.nx):(\d+)(?::(\d+))?/g, (all, f, l, c) => {
        const p = normRel(joinRel(dirName(runInfo.path), f.replace(/\\/g, '/')));
        return S.files.includes(p) ? `<a data-loc="${escHtml(p)}|${l}|${c || 1}">${all}</a>` : all;
      });
    }
    const cls = [term.color, term.bold ? 'ansi-1' : ''].filter(Boolean).join(' ');
    html += cls ? `<span class="${cls}">${x}</span>` : x;
  };
  while ((m = re.exec(text))) {
    flush(text.slice(last, m.index));
    for (const code of (m[1] || '0').split(';').map(Number)) {
      if (code === 0) { term.color = ''; term.bold = false; }
      else if (code === 1) term.bold = true;
      else if (code === 22) term.bold = false;
      else if (code === 39) term.color = '';
      else if (code in ANSI_CLASS) term.color = ANSI_CLASS[code];
    }
    last = re.lastIndex;
  }
  flush(text.slice(last));
  el.insertAdjacentHTML('beforeend', html);
  while (el.childNodes.length > 6000) el.firstChild.remove();
  if (atBottom) el.scrollTop = el.scrollHeight;
}

async function runProgram(kind) {
  if (!S.root) { toast("Ouvrez d'abord un dossier.", 'error'); return; }
  if (kind === 'run' && S.active && /^(html?|css|js)$/.test(extOf(S.active.path))) { startLive(); return; }
  if (!S.nxc) { toast('Compilateur nxc introuvable : indiquez son chemin dans les paramètres.', 'error'); return; }
  let path = findMainNx();
  if (!path) { toast('Aucun fichier NEXUL (.nx) à exécuter dans ce dossier.', 'error'); return; }
  // A module is run (or compiled) through the program that imports it.
  if (kind !== 'test') path = (await checkRoots(path))[0];
  if (S.run) await stopRun(true);
  await saveAll();
  showPanel('terminal');
  termClear();
  let r;
  try { r = await api.post('run?' + qs({ path, kind }), kind === 'run' ? settings.args : ''); } catch (e) { termWrite(`\x1b[31m${e.message}\x1b[0m\n`); return; }
  S.run = { id: r.id, from: 0, kind, path, started: performance.now() };
  updateRunUi();
  pollRun(S.run);
}

async function pollRun(runInfo) {
  if (S.run !== runInfo) return;
  let r;
  try { r = await api.get('run/poll?' + qs({ id: runInfo.id, from: runInfo.from })); } catch (e) { runInfo.timer = setTimeout(() => pollRun(runInfo), 300); return; }
  if (S.run !== runInfo) return;
  if (r.out) termWrite(r.out, runInfo);
  runInfo.from = r.next;
  if (r.done) { finishRun(runInfo, r.code, r.times || []); return; }
  runInfo.timer = setTimeout(() => pollRun(runInfo), 70);
}

/* Seconds with a decimal comma: 0,47 s. */
const secsFr = (s) => s.toFixed(2).replace('.', ',') + ' s';

function finishRun(runInfo, code, times) {
  S.run = null;
  // Compilation and execution are timed separately: the time spent by the program
  // (waiting for the keyboard, pauses…) is not the time of the compiler.
  const total = (performance.now() - runInfo.started) / 1000;
  const progTime = runInfo.kind === 'run' && times.length >= 2 ? times[1] : null;
  if (code === -9) termWrite(`\n\x1b[33m■ Programme arrêté.\x1b[0m\n`);
  else if (code === 0) {
    if (runInfo.kind === 'build') {
      const exe = absPath(joinRel(dirName(runInfo.path), stemOf(runInfo.path) + '.exe'));
      termWrite(`\n\x1b[32m✓ Exécutable créé : ${exe}\x1b[0m  `);
      $('.term-out').insertAdjacentHTML('beforeend', `<a data-reveal="${escHtml(joinRel(dirName(runInfo.path), stemOf(runInfo.path) + '.exe'))}">Afficher dans l'Explorateur</a>\n`);
      refreshTree();
    } else if (progTime !== null) {
      termWrite(`\n\x1b[32m✓ Programme terminé\x1b[0m\x1b[90m · exécution ${secsFr(progTime)} · compilation ${secsFr(times[0])}\x1b[0m\n`);
    } else termWrite(`\n\x1b[32m✓ Terminé en ${secsFr(times.length ? times.reduce((a, b) => a + b, 0) : total)}.\x1b[0m\n`);
  } else {
    termWrite(`\n\x1b[31m✗ Terminé avec le code ${code}.\x1b[0m\n`);
    scheduleCheck(runInfo.path, 10);
  }
  updateRunUi();
}

async function stopRun(silent) {
  const r = S.run;
  if (!r) return;
  try { await api.post('run/kill?' + qs({ id: r.id })); } catch (e) { /* already finished */ }
  if (silent) { clearTimeout(r.timer); S.run = null; updateRunUi(); }
}

function updateRunUi() {
  const running = !!S.run;
  const b = $('#btn-run');
  b.classList.toggle('danger', running);
  b.classList.toggle('primary', !running);
  b.innerHTML = running ? icon('stop') + '<span>Arrêter</span>' : icon('play') + '<span>Exécuter</span>';
  b.title = running ? 'Arrêter (Maj+F5)' : 'Exécuter (F5)';
  $('#term-stop').style.display = running ? '' : 'none';
  $('.term-input').classList.toggle('hidden', !(running && S.run.kind === 'run'));
  $('#run-state').textContent = running ? { run: 'Exécution en cours…', test: 'Tests en cours…', build: 'Compilation…' }[S.run.kind] : '';
  if (running && S.run.kind === 'run') setTimeout(() => $('.term-input input').focus(), 50);
  updateStatus();
}

async function editArgs() {
  const v = await promptBox('Arguments du programme', {
    label: 'Arguments passés au programme lancé avec F5', value: settings.args, placeholder: 'par exemple : 10 bonjour', hint: 'Séparez les arguments par des espaces.',
  });
  if (v === null) return;
  settings.args = v;
  saveSettings();
  toast(v ? `Arguments : ${v}` : 'Aucun argument.', 'success', 2000);
}

/* ==================================================================== live preview */

function buildPreview() {
  const pv = $('#preview');
  pv.innerHTML = `
  <div class="pv-toolbar">
    <div class="pv-title"><span class="live-dot on"></span>Aperçu</div>
    <div class="field pv-url"><input spellcheck="false" title="Page affichée — modifiez puis Entrée"></div>
    <div class="pv-devices">
      <button data-d="desktop" title="Ordinateur">${icon('desktop')}</button>
      <button data-d="tablet" title="Tablette (768 px)">${icon('tablet')}</button>
      <button data-d="phone" title="Téléphone (390 px)">${icon('phone')}</button>
    </div>
    <button class="icon-btn" id="pv-reload" title="Recharger">${icon('refresh')}</button>
    <button class="icon-btn" id="pv-open" title="Ouvrir dans le navigateur">${icon('external')}</button>
    <button class="icon-btn" id="pv-close" title="Fermer l'aperçu">${icon('close')}</button>
  </div>
  <div class="pv-stage"><div class="pv-frame-wrap"><iframe title="Aperçu de la page"></iframe></div></div>`;
  for (const b of $$('.pv-devices button', pv)) b.onclick = () => setDevice(b.dataset.d);
  $('#pv-reload').onclick = () => { const f = $('#preview iframe'); f.src = f.src; };
  $('#pv-open').onclick = () => openExternal();
  $('#pv-close').onclick = () => stopLive();
  const url = $('.pv-url input', pv);
  url.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); setPreview(normRel(url.value), true); }
  });
  setDevice(S.live.device);
}

function setDevice(d) {
  S.live.device = d;
  const wrap = $('.pv-frame-wrap');
  wrap.style.width = d === 'tablet' ? '768px' : d === 'phone' ? '390px' : '100%';
  wrap.classList.toggle('device', d !== 'desktop');
  for (const b of $$('.pv-devices button')) b.classList.toggle('on', b.dataset.d === d);
}

function findHtmlTarget() {
  const a = S.active;
  if (a && extOf(a.path) === 'html') return a.path;
  const dir = a ? dirName(a.path) : '';
  for (const c of [joinRel(dir, 'index.html'), 'index.html']) if (S.files.includes(c)) return c;
  return S.files.find((f) => extOf(f) === 'html') || null;
}

async function startLive(path) {
  if (!S.root) { toast("Ouvrez d'abord un dossier.", 'error'); return; }
  path = path || findHtmlTarget();
  if (!path) {
    const ok = await confirmBox('Aucune page HTML', "Ce dossier ne contient pas encore de page web. Créer un site de départ (index.html, style.css, script.js) ?", 'Créer le site');
    if (ok) await createSite();
    return;
  }
  await saveAll();
  S.live.on = true;
  $('#preview').classList.remove('hidden');
  $('#preview-resizer').classList.remove('hidden');
  setPreview(path, true);
  updateLiveUi();
}

function setPreview(path, force) {
  if (!force && S.live.path === path) return;
  S.live.path = path;
  $('#preview iframe').src = livePath(path);
  $('.pv-url input').value = '/' + path;
  updateLiveUi();
}

function stopLive() {
  if (!S.live.on) return;
  S.live.on = false;
  $('#preview iframe').src = 'about:blank';
  $('#preview').classList.add('hidden');
  $('#preview-resizer').classList.add('hidden');
  updateLiveUi();
}

function liveUrl() { return `http://127.0.0.1:${S.port}${livePath(S.live.path || findHtmlTarget() || 'index.html')}`; }

function openExternal() {
  if (!S.root) return;
  api.post('open-external', liveUrl());
}

function updateLiveUi() {
  $('#btn-live').classList.toggle('active', S.live.on);
  updateStatus();
  if (S.sidebarView === 'web') renderWebView();
}

/* ==================================================================== web designer view */

const HTML_BLOCKS = [
  ['Section d\'accueil', 'Titre, texte et bouton', '<section class="hero">\n\t<h1>${1:Un titre qui accroche}</h1>\n\t<p>${2:Une phrase qui présente votre projet.}</p>\n\t<a class="btn" href="${3:#}">${4:Commencer}</a>\n</section>'],
  ['Barre de navigation', 'Logo et liens', '<nav class="nav">\n\t<a class="logo" href="#">${1:MonSite}</a>\n\t<ul>\n\t\t<li><a href="#">Accueil</a></li>\n\t\t<li><a href="#">À propos</a></li>\n\t\t<li><a href="#">Contact</a></li>\n\t</ul>\n</nav>'],
  ['Carte', 'Image, titre et texte', '<article class="card">\n\t<img src="${1:https://picsum.photos/480/280}" alt="${2:Illustration}">\n\t<h3>${3:Titre de la carte}</h3>\n\t<p>${4:Une courte description.}</p>\n</article>'],
  ['Grille de cartes', 'Trois cartes côte à côte', '<div class="grid">\n\t<article class="card">\n\t\t<h3>${1:Rapide}</h3>\n\t\t<p>Une courte description.</p>\n\t</article>\n\t<article class="card">\n\t\t<h3>${2:Simple}</h3>\n\t\t<p>Une courte description.</p>\n\t</article>\n\t<article class="card">\n\t\t<h3>${3:Fiable}</h3>\n\t\t<p>Une courte description.</p>\n\t</article>\n</div>'],
  ['Bouton', 'Lien ou action', '<button class="btn">${1:Cliquez-moi}</button>'],
  ['Formulaire de contact', 'Nom, e-mail, message', '<form class="form">\n\t<label>Nom\n\t\t<input type="text" name="nom" required>\n\t</label>\n\t<label>E-mail\n\t\t<input type="email" name="email" required>\n\t</label>\n\t<label>Message\n\t\t<textarea name="message" rows="4"></textarea>\n\t</label>\n\t<button class="btn" type="submit">${1:Envoyer}</button>\n</form>'],
  ['Image', 'Avec texte alternatif', '<img src="${1:image.jpg}" alt="${2:Description de l\'image}">'],
  ['Liste', 'Liste à puces', '<ul>\n\t<li>${1:Premier élément}</li>\n\t<li>Deuxième élément</li>\n\t<li>Troisième élément</li>\n</ul>'],
  ['Tableau', 'En-tête et lignes', '<table>\n\t<thead>\n\t\t<tr><th>${1:Nom}</th><th>${2:Valeur}</th></tr>\n\t</thead>\n\t<tbody>\n\t\t<tr><td>A</td><td>1</td></tr>\n\t\t<tr><td>B</td><td>2</td></tr>\n\t</tbody>\n</table>'],
  ['Pied de page', 'Mentions', '<footer class="footer">\n\t<p>© ${1:2026} ${2:MonSite}. Tous droits réservés.</p>\n</footer>'],
];

const CSS_BLOCKS = [
  ['Centrer (flexbox)', 'Centre le contenu', 'display: flex;\njustify-content: center;\nalign-items: center;'],
  ['Grille adaptative', 'Colonnes automatiques', 'display: grid;\ngrid-template-columns: repeat(auto-fit, minmax(${1:240px}, 1fr));\ngap: ${2:24px};'],
  ['Dégradé', 'Arrière-plan coloré', 'background: linear-gradient(135deg, ${1:#8b5cf6}, ${2:#22d3ee});'],
  ['Ombre douce', 'Relief discret', 'box-shadow: 0 10px 30px rgba(0, 0, 0, 0.12);'],
  ['Coins arrondis', 'Bordure arrondie', 'border-radius: ${1:12px};'],
  ['Transition', 'Animation fluide', 'transition: ${1:all} 0.25s ease;'],
  ['Effet au survol', 'Soulève l\'élément', '.${1:card}:hover {\n\ttransform: translateY(-4px);\n\tbox-shadow: 0 14px 34px rgba(0, 0, 0, 0.18);\n}'],
  ['Apparition animée', 'Fondu à l\'arrivée', '@keyframes apparition {\n\tfrom { opacity: 0; transform: translateY(12px); }\n\tto { opacity: 1; transform: none; }\n}\n\n.${1:element} {\n\tanimation: apparition 0.6s ease both;\n}'],
  ['Version mobile', 'Écrans étroits', '@media (max-width: 700px) {\n\t$0\n}'],
];

const SWATCHES = ['#8b5cf6', '#6366f1', '#3b82f6', '#0ea5e9', '#06b6d4', '#14b8a6', '#22c55e', '#84cc16',
  '#eab308', '#f97316', '#ef4444', '#ec4899', '#111827', '#374151', '#9ca3af', '#ffffff'];

function renderWebView() {
  const view = $('.sb-view[data-view="web"]');
  view.innerHTML = '';
  const body = h('div', { class: 'sb-body' });
  view.append(h('div', { class: 'sb-header' }, h('span', { text: 'Concepteur web' })), body);
  if (!S.root) {
    body.append(h('div', { class: 'sb-empty' }, h('p', { text: 'Ouvrez un dossier ou créez un site pour concevoir des pages web avec un aperçu en direct.' }),
      h('button', { class: 'btn primary block', html: icon('web') + 'Créer un site web', onclick: () => run('newSite') }),
      h('button', { class: 'btn block', html: icon('folderOpen') + 'Ouvrir un dossier', onclick: () => run('openFolder') })));
    return;
  }
  const on = S.live.on;
  body.append(h('div', { class: 'web-card' },
    h('h3', {}, h('span', { class: 'live-dot' + (on ? ' on' : '') }), on ? 'Serveur en direct actif' : 'Serveur en direct'),
    h('p', { text: on ? "La page se recharge à chaque enregistrement (automatique pendant la frappe)." : "Affiche vos pages HTML à côté du code et les recharge à chaque modification." }),
    on ? h('div', { class: 'url-chip', title: 'Adresse locale', text: liveUrl() }) : null,
    h('button', { class: 'btn primary block', html: icon(on ? 'stop' : 'live') + (on ? "Arrêter l'aperçu" : "Démarrer l'aperçu"), onclick: () => run('togglePreview') }),
    h('button', { class: 'btn block', html: icon('external') + 'Ouvrir dans le navigateur', onclick: () => openExternal() })));
  if (!S.files.some((f) => extOf(f) === 'html')) {
    body.append(h('div', { class: 'web-card' }, h('h3', { html: icon('sparkle') + 'Nouveau site' }),
      h('p', { text: 'Crée index.html, style.css et script.js avec une page de départ soignée.' }),
      h('button', { class: 'btn block', html: icon('plus') + 'Créer le site', onclick: () => createSite() })));
  }
  const blockList = (items, lang) => h('div', { class: 'snippet-list' }, items.map(([name, detail, code]) => h('div', {
    class: 'snippet-item', title: code.replace(/\$\{\d+:?([^}]*)\}/g, '$1').replace(/\$\d/g, ''),
    onclick: () => insertBlock(code, lang),
  }, h('code', { text: name }), h('span', { text: detail }))));
  body.append(h('div', { class: 'web-card' }, h('h3', { html: icon('code') + 'Blocs HTML' }),
    h('p', { text: "Cliquez pour insérer au curseur d'un fichier HTML." }), blockList(HTML_BLOCKS, 'html')));
  body.append(h('div', { class: 'web-card' }, h('h3', { html: icon('wand') + 'Styles CSS' }),
    h('p', { text: "Cliquez pour insérer au curseur d'un fichier CSS." }), blockList(CSS_BLOCKS, 'css')));
  body.append(h('div', { class: 'web-card' }, h('h3', { html: icon('palette') + 'Couleurs' }),
    h('p', { text: 'Cliquez pour insérer la couleur (et la copier). Dans le CSS, cliquez sur le carré à droite d\'une couleur pour la modifier.' }),
    h('div', { class: 'swatches' }, SWATCHES.map((c) => h('div', { class: 'swatch', title: c, style: { background: c }, onclick: () => insertColor(c) })))));
}

function insertBlock(code, lang) {
  const ed = S.active && S.active.ed;
  const ok = ed && (ed.lang === lang || (lang === 'css' && ed.lang === 'html'));
  if (!ok) { toast(lang === 'html' ? "Ouvrez d'abord un fichier HTML." : "Ouvrez d'abord un fichier CSS.", 'error'); return; }
  ed.insertAtCursor(code);
}

function insertColor(c) {
  copyText(c);
  const ed = S.active && S.active.ed;
  if (ed && (ed.lang === 'css' || ed.lang === 'html' || ed.lang === 'js')) ed.insertAtCursor(c);
}

/* ==================================================================== projects */

const SITE_HTML = (name) => `<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>${name}</title>
    <link rel="stylesheet" href="style.css">
</head>
<body>
    <header class="hero">
        <p class="badge">Créé avec NEXUL Studio</p>
        <h1>Bienvenue sur <span>${name}</span></h1>
        <p>Modifiez <code>index.html</code> ou <code>style.css</code> : l'aperçu se met à jour pendant que vous écrivez.</p>
        <button id="bouton" class="btn">Cliquez-moi</button>
        <p id="compteur" class="compteur"></p>
    </header>

    <main class="grid">
        <article class="card">
            <h3>Rapide</h3>
            <p>L'aperçu en direct se recharge à chaque modification.</p>
        </article>
        <article class="card">
            <h3>Simple</h3>
            <p>Tapez <code>section</code> puis Tab pour créer une balise.</p>
        </article>
        <article class="card">
            <h3>Coloré</h3>
            <p>Cliquez sur le carré à droite d'une couleur CSS pour la changer.</p>
        </article>
    </main>

    <script src="script.js"></script>
</body>
</html>
`;

const SITE_CSS = `* {
    box-sizing: border-box;
}

body {
    margin: 0;
    font-family: "Segoe UI", system-ui, sans-serif;
    color: #1f2335;
    background: #f6f7fb;
}

.hero {
    padding: 80px 24px 64px;
    text-align: center;
    color: #ffffff;
    background: linear-gradient(135deg, #8b5cf6, #22d3ee);
}

.hero h1 {
    margin: 12px 0;
    font-size: 44px;
}

.hero h1 span {
    text-decoration: underline;
    text-underline-offset: 6px;
}

.badge {
    display: inline-block;
    margin: 0;
    padding: 4px 12px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.2);
    font-size: 13px;
}

.btn {
    margin-top: 18px;
    padding: 12px 26px;
    border: 0;
    border-radius: 10px;
    background: #ffffff;
    color: #6d28d9;
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
    transition: transform 0.2s ease, box-shadow 0.2s ease;
}

.btn:hover {
    transform: translateY(-2px);
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.18);
}

.compteur {
    min-height: 22px;
}

.grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 24px;
    max-width: 960px;
    margin: -32px auto 48px;
    padding: 0 24px;
}

.card {
    padding: 24px;
    border-radius: 14px;
    background: #ffffff;
    box-shadow: 0 10px 30px rgba(31, 35, 53, 0.08);
}

.card h3 {
    margin-top: 0;
    color: #6d28d9;
}

code {
    padding: 2px 6px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.08);
}
`;

const SITE_JS = `// Ce script s'exécute dans la page.
const bouton = document.querySelector('#bouton');
const compteur = document.querySelector('#compteur');
let clics = 0;

bouton.addEventListener('click', () => {
    clics += 1;
    compteur.textContent = \`Vous avez cliqué \${clics} fois.\`;
});
`;

const NX_MAIN = (name) => `// ${name} — un programme NEXUL.
// F5 : exécuter   ·   F6 : lancer les tests   ·   F7 : vérifier

fn carre(x: i64) -> i64 {
    return x * x
}

fn main() {
    let nom = "${name.replace(/[{}"\\]/g, '')}"
    println("Bonjour depuis {nom} !")
    for i in 1..6 {
        println("{i} au carré = {carre(i)}")
    }
}

@test
fn carre_fonctionne() {
    assert(carre(3) == 9)
    assert(carre(-4) == 16)
}
`;

async function createFiles(files) {
  let st = null;
  for (const [path, content] of files) {
    if (S.files.includes(path)) continue;
    st = await api.post('new?' + qs({ path, kind: 'file' }), content);
  }
  if (st) applyState(st);
}

async function createSite() {
  if (!S.root) { await newProject('web'); return; }
  const name = S.rootName || 'Mon site';
  try {
    await createFiles([['index.html', SITE_HTML(escHtml(name))], ['style.css', SITE_CSS], ['script.js', SITE_JS]]);
  } catch (e) { toast(e.message, 'error'); return; }
  await openFile('style.css', { background: true });
  await openFile('index.html');
  await startLive('index.html');
  showView('web');
}

function newProject(kind = 'nexul') {
  return new Promise(async (resolve) => {
    let info = { desktop: '', home: '' };
    try { info = await api.get('browse?' + qs({ path: '' })); } catch (e) { /* ignore */ }
    let location = settings.projectDir || info.desktop || info.home;
    const name = h('input', { spellcheck: 'false' });
    name.value = kind === 'web' ? 'mon-site' : 'mon-projet';
    const loc = h('input', { spellcheck: 'false' });
    loc.value = location;
    const hint = h('div', { class: 'hint' });
    const upd = () => { hint.textContent = 'Le projet sera créé dans : ' + loc.value.replace(/[\\/]+$/, '') + '\\' + name.value.trim(); };
    name.addEventListener('input', upd);
    loc.addEventListener('input', upd);
    upd();
    const choice = (k, ic, title, text) => h('div', { class: 'choice' + (kind === k ? ' on' : ''), dataset: { k }, onclick: (e) => {
      kind = k;
      for (const c of $$('.choice', e.currentTarget.parentElement)) c.classList.toggle('on', c.dataset.k === k);
      if (k === 'web' && name.value === 'mon-projet') name.value = 'mon-site';
      if (k === 'nexul' && name.value === 'mon-site') name.value = 'mon-projet';
      upd();
    } }, h('b', { html: icon(ic) + title }), h('span', { text }));
    const create = async (st) => {
      const n = name.value.trim();
      if (!n || /[\\/:*?"<>|]/.test(n)) { toast('Nom de projet invalide.', 'error'); return; }
      const base = loc.value.trim().replace(/[\\/]+$/, '');
      if (!base) { toast('Choisissez un emplacement.', 'error'); return; }
      if (!(await closeAllDocs())) return;
      let state;
      try { state = await api.post('create-project', { path: base + '\\' + n }); } catch (e) { toast(e.message, 'error'); return; }
      st.close();
      settings.projectDir = base;
      applyState(state);
      rememberRoot();
      try {
        if (kind === 'web') {
          await createFiles([['index.html', SITE_HTML(escHtml(n))], ['style.css', SITE_CSS], ['script.js', SITE_JS]]);
          await openFile('style.css', { background: true });
          await openFile('index.html');
          await startLive('index.html');
          showView('web');
        } else {
          await createFiles([['main.nx', NX_MAIN(n)]]);
          await openFile('main.nx');
          showView('explorer');
        }
      } catch (e) { toast(e.message, 'error'); }
      toast(`Projet « ${n} » créé.`, 'success');
      resolve(true);
    };
    const st = openModal({
      title: 'Nouveau projet', icon: 'sparkle', wide: true,
      body: [
        h('div', { class: 'choice-grid', style: { marginBottom: '16px' } },
          choice('nexul', 'code', 'Programme NEXUL', 'Un fichier main.nx prêt à exécuter (F5), avec un exemple de test (F6).'),
          choice('web', 'web', 'Site web', 'index.html, style.css et script.js, avec aperçu en direct.')),
        h('div', { class: 'form-row' }, h('label', { text: 'Nom du projet' }), h('div', { class: 'field' }, name)),
        h('div', { class: 'form-row' }, h('label', { text: 'Emplacement' }),
          h('div', { style: { display: 'flex', gap: '8px' } }, h('div', { class: 'field', style: { flex: '1' } }, loc),
            h('button', { class: 'btn', text: 'Parcourir…', onclick: async () => { const p = await pickFolder({ title: "Choisir l'emplacement", ok: 'Choisir', start: loc.value }); if (p) { loc.value = p; upd(); } } })),
          hint),
      ],
      buttons: [{ label: 'Annuler', onclick: (s) => { s.close(); resolve(false); } }, { label: 'Créer le projet', primary: true, onclick: (s) => create(s) }],
      onCancel: (s) => { s.close(); resolve(false); },
    });
    name.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); create(st); } });
    setTimeout(() => { name.focus(); name.select(); }, 0);
  });
}

function pickFolder({ title = 'Ouvrir un dossier', ok = 'Ouvrir ce dossier', start } = {}) {
  return new Promise((resolve) => {
    let cur = '';
    const pathInput = h('input', { spellcheck: 'false', placeholder: 'Chemin du dossier' });
    const list = h('div', { class: 'folder-list' });
    const places = h('div', { class: 'quick-places' });
    let placesBuilt = false;
    const loadDir = async (p) => {
      let info;
      try { info = await api.get('browse?' + qs({ path: p })); } catch (e) { toast(e.message, 'error'); return false; }
      cur = info.path;
      pathInput.value = cur;
      list.innerHTML = '';
      if (cur) list.append(h('div', { class: 'folder-row', onclick: () => loadDir(info.parent) }, iconEl('collapse'), h('span', { text: '.. (dossier parent)' })));
      for (const d of info.dirs) list.append(h('div', { class: 'folder-row', title: d.path, onclick: () => loadDir(d.path), ondblclick: () => accept(d.path) }, iconEl('folder'), h('span', { text: d.name })));
      if (!info.dirs.length) list.append(h('div', { class: 'prob-empty', text: cur ? 'Aucun sous-dossier.' : 'Aucun lecteur.' }));
      if (!placesBuilt) {
        placesBuilt = true;
        places.append(
          h('button', { class: 'btn', html: icon('desktop') + 'Bureau', onclick: () => loadDir(info.desktop) }),
          h('button', { class: 'btn', html: icon('folder') + 'Dossier personnel', onclick: () => loadDir(info.home) }),
          h('button', { class: 'btn', html: icon('layout') + 'Ce PC', onclick: () => loadDir('') }));
      }
      return true;
    };
    let st;
    const accept = (p) => {
      const target = p || cur;
      if (!target) { toast('Choisissez un dossier.', 'error'); return; }
      st.close();
      settings.lastBrowse = target;
      saveSettings();
      resolve(target);
    };
    st = openModal({
      title, icon: 'folderOpen', wide: true,
      body: [places, h('div', { class: 'field', style: { marginTop: '10px', height: '32px' } }, pathInput), list,
        h('div', { class: 'hint', style: { marginTop: '8px', fontSize: '12px', color: 'var(--fg-faint)' }, text: 'Cliquez pour entrer dans un dossier, double-cliquez pour le choisir directement.' })],
      buttons: [{ label: 'Annuler', onclick: (s) => { s.close(); resolve(null); } }, { label: ok, primary: true, onclick: () => accept() }],
      onCancel: (s) => { s.close(); resolve(null); },
    });
    pathInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); loadDir(pathInput.value.trim()); } });
    (async () => {
      if (!(await loadDir(start || settings.lastBrowse || ''))) await loadDir('');
      if (!start && !settings.lastBrowse) {
        const info = await api.get('browse?' + qs({ path: '' })).catch(() => null);
        if (info && info.desktop) await loadDir(info.desktop);
      }
    })();
  });
}

/* ==================================================================== search view */

function buildSearchView() {
  const view = $('.sb-view[data-view="search"]');
  const input = h('input', { id: 'search-input', placeholder: 'Rechercher dans les fichiers', spellcheck: 'false' });
  const caseBtn = h('button', { class: 'toggle', title: 'Respecter la casse', text: 'Aa' });
  const summary = h('div', { class: 'search-summary' });
  const results = h('div', { class: 'sb-body' });
  view.append(h('div', { class: 'sb-header' }, h('span', { text: 'Rechercher' })),
    h('div', { class: 'search-box' }, h('div', { class: 'field' }, input, caseBtn)), summary, results);
  let timer = 0, seq = 0;
  const doSearch = async () => {
    const q = input.value;
    const my = ++seq;
    if (!q || !S.root) { results.innerHTML = ''; summary.textContent = S.root ? '' : 'Aucun dossier ouvert.'; return; }
    let res;
    try { res = await api.get('search?' + qs({ q, case: S.searchCase ? '1' : '0' })); } catch (e) { summary.textContent = e.message; return; }
    if (my !== seq) return;
    const groups = new Map();
    for (const r of res) { if (!groups.has(r.path)) groups.set(r.path, []); groups.get(r.path).push(r); }
    summary.textContent = res.length ? `${res.length}${res.length >= 1000 ? '+' : ''} résultat${res.length > 1 ? 's' : ''} dans ${groups.size} fichier${groups.size > 1 ? 's' : ''}` : 'Aucun résultat.';
    results.innerHTML = '';
    const qlen = [...q].length;
    for (const [path, hits] of groups) {
      results.append(h('div', { class: 'search-file', title: path, onclick: () => openFile(path) },
        fileIconEl(baseName(path)), h('span', { text: baseName(path) }), h('span', { class: 'dir', text: dirName(path) }), h('span', { class: 'count', text: hits.length })));
      for (const r of hits) {
        const t = r.text;
        const a = cpIndex(t, r.col), b = cpIndex(t, r.col + qlen);
        const lead = Math.max(0, a - 24);
        const pre = (lead > 0 ? '…' : '') + t.slice(lead, a).trimStart();
        results.append(h('div', { class: 'search-hit', title: `ligne ${r.line}`, onclick: () => openFile(path, { line: r.line - 1, charCol: r.col, len: b - a }) },
          h('span', { html: escHtml(pre) + '<mark>' + escHtml(t.slice(a, b)) + '</mark>' + escHtml(t.slice(b, b + 80)) })));
      }
    }
  };
  input.addEventListener('input', () => { clearTimeout(timer); timer = setTimeout(doSearch, 250); });
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') { clearTimeout(timer); doSearch(); } });
  caseBtn.addEventListener('click', () => { S.searchCase = !S.searchCase; caseBtn.classList.toggle('on', S.searchCase); doSearch(); });
}

/* ==================================================================== status bar */

function buildStatusBar() {
  const sb = $('#statusbar');
  sb.innerHTML = '';
  const item = (id, cls, title, onclick) => h('div', { class: 'sb-item' + (onclick ? ' clickable' : '') + (cls ? ' ' + cls : ''), id, title, onclick });
  sb.append(
    item('sb-brand', 'brand', 'À propos de NEXUL Studio', () => run('about')),
    item('sb-diags', '', 'Problèmes (Ctrl+Maj+M)', () => run('showProblems')),
    item('sb-run', '', ''),
    h('div', { class: 'sb-spacer' }),
    item('sb-cursor', '', 'Aller à la ligne (Ctrl+G)', () => run('gotoLine')),
    item('sb-indent', '', "Taille de l'indentation", () => { settings.tabSize = settings.tabSize === 4 ? 2 : 4; saveSettings(); applySettings(); toast(`Indentation : ${settings.tabSize} espaces.`, 'success', 1800); }),
    item('sb-enc', '', 'Encodage'),
    item('sb-eol', '', 'Fins de ligne'),
    item('sb-lang', '', 'Langage du fichier'),
    item('sb-live', '', 'Aperçu web en direct (Alt+L)', () => run('togglePreview')),
    item('sb-nxc', '', '', () => run('settings')),
  );
  $('#sb-brand').innerHTML = '<span style="letter-spacing:.06em">NEXUL</span>';
}

function updateCursorStatus() {
  const el = $('#sb-cursor');
  if (!el) return;
  const ed = S.active && S.active.ed;
  if (!ed) { el.style.display = 'none'; return; }
  el.style.display = '';
  const c = ed.cursorInfo();
  el.textContent = `Ligne ${c.line}, col. ${c.col}` + (c.sel ? ` (${c.sel} sélectionné${c.sel > 1 ? 's' : ''})` : '');
}

function updateStatus() {
  if (!$('#sb-diags')) return;
  const t = diagTotals();
  $('#sb-diags').innerHTML = icon('error', t.err ? 'ic-err' : '') + `<span>${t.err}</span>` + icon('warning', t.warn ? 'ic-warn' : '') + `<span>${t.warn}</span>`;
  const r = $('#sb-run');
  r.innerHTML = S.run ? icon('play', 'ic-ok') + `<span>${{ run: 'Exécution…', test: 'Tests…', build: 'Compilation…' }[S.run.kind]}</span>` : '';
  r.style.display = S.run ? '' : 'none';
  const ed = S.active && S.active.ed;
  for (const id of ['sb-indent', 'sb-enc', 'sb-eol', 'sb-lang']) $('#' + id).style.display = ed ? '' : 'none';
  if (ed) {
    $('#sb-indent').textContent = `Espaces : ${settings.tabSize}`;
    $('#sb-enc').textContent = 'UTF-8';
    $('#sb-eol').textContent = ed.eol === '\r\n' ? 'CRLF' : 'LF';
    $('#sb-lang').innerHTML = (LANGS[ed.lang] || LANGS.text).name;
  }
  const live = $('#sb-live');
  live.classList.toggle('live-on', S.live.on);
  live.innerHTML = icon('live') + `<span>${S.live.on ? `En direct · port ${S.port}` : 'Aperçu'}</span>`;
  live.style.display = S.root ? '' : 'none';
  const nx = $('#sb-nxc');
  nx.innerHTML = S.nxc ? icon('check') + '<span>nxc</span>' : icon('warning', 'ic-warn') + '<span>nxc introuvable</span>';
  nx.title = S.nxc ? `Compilateur : ${S.nxc}` : 'Compilateur NEXUL introuvable — cliquez pour indiquer son chemin';
  updateCursorStatus();
}

/* ==================================================================== settings, about, shortcuts */

function openSettings() {
  const sw = (key, onChange) => {
    const el = h('button', { class: 'switch' + (settings[key] ? ' on' : ''), role: 'switch', 'aria-checked': String(!!settings[key]) });
    el.onclick = () => {
      settings[key] = !settings[key];
      el.classList.toggle('on', settings[key]);
      el.setAttribute('aria-checked', String(settings[key]));
      saveSettings();
      applySettings();
      if (onChange) onChange();
    };
    return el;
  };
  const seg = (key, options) => {
    const box = h('div', { class: 'seg' });
    for (const [v, label] of options) {
      box.append(h('button', {
        class: settings[key] === v ? 'on' : '', text: label,
        onclick: (e) => {
          settings[key] = v;
          for (const b of box.children) b.classList.toggle('on', b === e.currentTarget);
          saveSettings();
          applySettings();
          renderActivityTheme();
        },
      }));
    }
    return box;
  };
  const sizes = h('select', { class: 'select' });
  for (let s = 11; s <= 24; s++) sizes.append(h('option', { value: s, text: s + ' px', selected: s === settings.fontSize }));
  sizes.onchange = () => setFontSize(+sizes.value);
  const nxcInput = h('input', { spellcheck: 'false', placeholder: 'C:\\NEXUL\\target\\release\\nxc.exe' });
  nxcInput.value = S.nxc || '';
  const setting = (title, desc, control) => h('div', { class: 'setting' }, h('div', { class: 's-label' }, h('b', { text: title }), h('span', { text: desc })), control);
  const st = openModal({
    title: 'Paramètres', icon: 'gear', wide: true,
    body: [
      setting('Thème', "Couleurs de l'interface et du code.", seg('theme', [['dark', 'Sombre'], ['light', 'Clair']])),
      setting('Taille du texte', "Taille du code (aussi : Ctrl+molette).", sizes),
      setting('Indentation', "Nombre d'espaces par niveau.", seg('tabSize', [[2, '2 espaces'], [4, '4 espaces']])),
      setting('Enregistrement automatique', 'Enregistre les fichiers pendant que vous tapez.', sw('autoSave')),
      setting('Vérifier à l\'enregistrement', 'Analyse les fichiers NEXUL et souligne les erreurs.', sw('checkOnSave')),
      setting('Fermeture automatique', 'Ajoute ) ] } " \' après leur ouverture.', sw('autoClose')),
      setting('Surligner le mot courant', 'Montre les autres occurrences du mot sous le curseur.', sw('wordHighlight')),
      setting('Minimap', 'Vue miniature du fichier à droite.', sw('minimap')),
      h('div', { class: 'setting', style: { flexDirection: 'column', alignItems: 'stretch' } },
        h('div', { class: 's-label' }, h('b', { text: 'Compilateur NEXUL (nxc)' }), h('span', { text: S.nxc ? 'Trouvé automatiquement.' : 'Introuvable : indiquez le chemin de nxc.exe.' })),
        h('div', { style: { display: 'flex', gap: '8px', marginTop: '8px' } }, h('div', { class: 'field', style: { flex: '1', height: '30px' } }, nxcInput),
          h('button', { class: 'btn', text: 'Appliquer', onclick: async () => {
            try { applyState(await api.post('set-nxc', { path: nxcInput.value.trim() })); toast('Compilateur configuré.', 'success'); } catch (e) { toast(e.message, 'error'); }
          } }))),
    ],
    buttons: [{ label: 'Fermer', primary: true, onclick: (s) => s.close() }],
  });
  setTimeout(() => st.primary && st.primary.focus(), 0);
}

function showAbout() {
  const st = openModal({
    title: 'NEXUL Studio', icon: 'sparkle',
    body: [
      h('div', { style: { display: 'flex', gap: '16px', alignItems: 'center', marginBottom: '12px' } },
        h('div', { class: 'welcome-logo', style: { width: '56px', height: '56px', fontSize: '28px', borderRadius: '14px' }, text: 'N' }),
        h('div', {}, h('div', { style: { fontWeight: '700', fontSize: '16px', color: 'var(--fg-strong)' }, text: 'NEXUL Studio 0.1' }),
          h('div', { style: { color: 'var(--fg-muted)' }, text: 'Langage NEXUL 0.1 « Noyau »' }))),
      h('p', { text: "Éditeur local pour le langage NEXUL : coloration, vérification continue, exécution, tests, compilation en exécutable, et conception web avec aperçu en direct." }),
      h('p', { text: `Compilateur : ${S.nxc || 'introuvable'}` }),
      h('p', { text: `Serveur local : http://127.0.0.1:${S.port}` }),
    ],
    buttons: [{ label: 'Fermer', primary: true, onclick: (s) => s.close() }],
  });
  setTimeout(() => st.primary && st.primary.focus(), 0);
}

function showShortcuts() {
  const rows = [
    ['Exécuter le programme', 'F5'], ['Arrêter', 'Maj+F5'], ['Lancer les tests', 'F6'], ['Vérifier', 'F7'], ['Compiler en .exe', 'Ctrl+Maj+B'],
    ['Aperçu web en direct', 'Alt+L'], ['Ouvrir un fichier', 'Ctrl+P'], ['Palette de commandes', 'Ctrl+Maj+P'], ['Aller à la ligne', 'Ctrl+G'],
    ['Enregistrer', 'Ctrl+S'], ['Rechercher / remplacer', 'Ctrl+F / Ctrl+H'], ['Rechercher dans les fichiers', 'Ctrl+Maj+F'],
    ['Suggestions', 'Ctrl+Espace'], ['Commenter', 'Ctrl+/ ou Ctrl+:'], ['Dupliquer la ligne', 'Maj+Alt+↓'], ['Déplacer la ligne', 'Alt+↑ / Alt+↓'],
    ['Supprimer la ligne', 'Ctrl+Maj+K'], ['Indenter / désindenter', 'Tab / Maj+Tab'], ['Nouvelle ligne dessous', 'Ctrl+Entrée'],
    ['Développer une balise HTML (div, .classe, !)', 'Tab'], ['Terminal', 'Ctrl+ù'], ['Barre latérale', 'Ctrl+B'], ['Taille du texte', 'Ctrl+= / Ctrl+-'],
  ];
  const table = h('table', { class: 'shortcut-table' }, rows.map(([a, b]) => h('tr', {}, h('td', { text: a }), h('td', {}, ...b.split(' / ').map((k, i) => [i ? ' / ' : '', h('span', { class: 'kbd', text: k })])))));
  const st = openModal({ title: 'Raccourcis clavier', icon: 'keyboard', wide: true, body: table, buttons: [{ label: 'Fermer', primary: true, onclick: (s) => s.close() }] });
  setTimeout(() => st.primary && st.primary.focus(), 0);
}

/* ==================================================================== start */

async function externalChanges() {
  await refreshTree();
  for (const d of S.docs) {
    if (!d.ed || d.ed.isDirty()) continue;
    let text;
    try { text = await api.get('file?' + qs({ path: d.path })); } catch (e) { continue; }
    if (text.replace(/\r\n?/g, '\n') !== d.ed.savedValue) d.ed.setText(text);
  }
}

async function init() {
  await loadKv();
  buildLayout();
  applySettings();
  renderActivityTheme();
  updateRunUi();
  let st;
  try { st = await api.get('state'); } catch (e) {
    document.body.innerHTML = '<div style="display:grid;place-items:center;height:100vh;font-family:Segoe UI;color:#c0caf5;background:#13141c">Impossible de joindre NEXUL Studio. Relancez l\'application.</div>';
    return;
  }
  applyState(st);
  if (!S.root && settings.lastRoot) {
    try { applyState(await api.post('open-folder', { path: settings.lastRoot })); } catch (e) { /* folder moved */ }
  }
  if (S.root) { rememberRoot(); await restoreSession(); }
  renderWelcome();
  updateStatus();
  api.post('ping').catch(() => {});
  setInterval(() => api.post('ping').catch(() => {}), 3000);
  setInterval(() => { if (document.hasFocus() && S.root && !S.inline) refreshTree(); }, 4000);
  addEventListener('focus', () => { if (S.root) externalChanges(); });
}

init();
