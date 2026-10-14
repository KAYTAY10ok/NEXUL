'use strict';
/* NEXUL Studio — extensions (API version 1, see docs/studio/extensions.md).
 *
 * An extension is a folder with `plugin.json` and a script. The script runs in a Web
 * Worker: it has no access to the interface, the files or the network of the Studio,
 * only to the `nexul` object, whose functions check the permissions declared in the
 * manifest and accepted by the user. */

const PLUGIN_API_VERSION = '1';

const PERMISSIONS = {
  'commands': 'ajouter des commandes (palette et menu Extensions)',
  'notifications': 'afficher des notifications',
  'statusbar': 'afficher un texte dans la barre d\'état',
  'editor.read': 'lire le fichier ouvert et la sélection',
  'editor.write': 'modifier le fichier ouvert',
  'workspace.read': 'lire les fichiers du dossier ouvert',
  'workspace.write': 'créer et modifier des fichiers du dossier ouvert',
  'compiler': 'utiliser le compilateur NEXUL (vérifier, mettre en forme)',
};

/* The `nexul` object inside the worker. */
const PLUGIN_PRELUDE = `
'use strict';
(() => {
  let seq = 0;
  const pending = new Map();
  const handlers = new Map();
  const listeners = { save: [], open: [], change: [], activate: [] };
  const call = (method, ...args) => new Promise((resolve, reject) => {
    const id = ++seq;
    pending.set(id, { resolve, reject });
    postMessage({ type: 'call', id, method, args });
  });
  self.onmessage = async (e) => {
    const m = e.data;
    if (m.type === 'result') {
      const p = pending.get(m.id);
      if (!p) return;
      pending.delete(m.id);
      if (m.error) p.reject(new Error(m.error)); else p.resolve(m.value);
    } else if (m.type === 'command') {
      const h = handlers.get(m.id);
      try { if (h) await h(); } catch (err) { postMessage({ type: 'log', level: 'error', text: String(err && err.stack || err) }); }
    } else if (m.type === 'event') {
      for (const h of listeners[m.name] || []) { try { await h(m.data); } catch (err) { postMessage({ type: 'log', level: 'error', text: String(err) }); } }
    }
  };
  self.nexul = Object.freeze({
    apiVersion: '${PLUGIN_API_VERSION}',
    commands: Object.freeze({
      register(id, title, handler) { handlers.set(id, handler); return call('commands.register', id, title); },
      execute(id) { return call('commands.execute', id); },
    }),
    window: Object.freeze({
      showMessage(text, kind) { return call('window.showMessage', String(text), kind || 'info'); },
      setStatus(text) { return call('window.setStatus', String(text)); },
      prompt(title, value) { return call('window.prompt', String(title), value === undefined ? '' : String(value)); },
    }),
    editor: Object.freeze({
      getText() { return call('editor.getText'); },
      getSelection() { return call('editor.getSelection'); },
      getPath() { return call('editor.getPath'); },
      getLanguage() { return call('editor.getLanguage'); },
      replaceSelection(text) { return call('editor.replaceSelection', String(text)); },
      insert(text) { return call('editor.replaceSelection', String(text)); },
      setText(text) { return call('editor.setText', String(text)); },
    }),
    workspace: Object.freeze({
      listFiles() { return call('workspace.listFiles'); },
      readFile(path) { return call('workspace.readFile', String(path)); },
      writeFile(path, text) { return call('workspace.writeFile', String(path), String(text)); },
    }),
    compiler: Object.freeze({
      check(path) { return call('compiler.check', String(path)); },
      format(text) { return call('compiler.format', String(text)); },
    }),
    events: Object.freeze({
      on(name, handler) { if (listeners[name]) listeners[name].push(handler); else throw new Error('événement inconnu : ' + name); },
    }),
    log(...args) { postMessage({ type: 'log', level: 'info', text: args.map(String).join(' ') }); },
  });
})();
`;

const PLUGINS = {
  list: [],
  running: new Map(), // id -> { worker, manifest, commands: Map }
  status: new Map(),  // id -> status text

  enabledOf(id, builtin) {
    const st = (settings.plugins || {})[id];
    return st ? !!st.enabled : false;
  },

  setEnabled(id, on, perms) {
    settings.plugins = settings.plugins || {};
    settings.plugins[id] = { enabled: on, permissions: perms || [] };
    saveSettings();
  },

  async load() {
    try { this.list = await api.get('plugins'); } catch (e) { this.list = []; }
    for (const p of this.list) {
      const st = (settings.plugins || {})[p.id];
      // Accepted permissions: those of the manifest when it was enabled.
      if (st && st.enabled && this.samePerms(st.permissions, p.manifest.permissions)) this.start(p);
    }
    this.render();
  },

  samePerms(a, b) {
    const x = [...(a || [])].sort().join(','), y = [...(b || [])].sort().join(',');
    return x === y;
  },

  async start(p) {
    if (this.running.has(p.id)) return;
    const m = p.manifest;
    if (String(m.apiVersion || '1') !== PLUGIN_API_VERSION) { this.status.set(p.id, `API ${m.apiVersion} non prise en charge`); return; }
    let code;
    try { code = await api.get('plugin-file?' + qs({ id: p.id, file: m.main || 'main.js' })); } catch (e) { this.status.set(p.id, e.message); return; }
    const blob = new Blob([PLUGIN_PRELUDE, '\n', code, `\n//# sourceURL=plugin:${p.id}/${m.main || 'main.js'}`], { type: 'text/javascript' });
    const url = URL.createObjectURL(blob);
    const worker = new Worker(url);
    URL.revokeObjectURL(url);
    const entry = { worker, manifest: m, commands: new Map(), perms: new Set(m.permissions || []) };
    this.running.set(p.id, entry);
    this.status.set(p.id, 'active');
    worker.onmessage = (e) => this.onMessage(p.id, entry, e.data);
    worker.onerror = (e) => { this.status.set(p.id, 'erreur : ' + e.message); toast(`Extension ${m.name || p.id} : ${e.message}`, 'error'); this.render(); };
  },

  stop(id) {
    const e = this.running.get(id);
    if (!e) return;
    e.worker.terminate();
    for (const cid of e.commands.keys()) delete CMD['plugin:' + id + ':' + cid];
    this.running.delete(id);
    this.status.delete(id);
    const item = $(`#sb-plugin-${CSS.escape(id)}`);
    if (item) item.remove();
  },

  async onMessage(id, entry, m) {
    if (m.type === 'log') { (m.level === 'error' ? console.error : console.log)(`[${id}]`, m.text); return; }
    if (m.type !== 'call') return;
    const reply = (value, error) => entry.worker.postMessage({ type: 'result', id: m.id, value, error });
    try { reply(await this.api(id, entry, m.method, m.args || [])); } catch (e) { reply(undefined, e.message || String(e)); }
  },

  need(entry, perm) {
    if (!entry.perms.has(perm)) throw new Error(`permission « ${perm} » non déclarée dans plugin.json`);
  },

  /* The functions of the API, with their permissions. */
  async api(id, entry, method, args) {
    const ed = () => {
      if (!S.active || !S.active.ed) throw new Error('aucun fichier ouvert');
      return S.active.ed;
    };
    const safePath = (p) => {
      const n = normRel(String(p).replace(/\\/g, '/'));
      if (!n || n.startsWith('..') || /^[a-z]:/i.test(n)) throw new Error('chemin hors du dossier ouvert');
      return n;
    };
    switch (method) {
      case 'commands.register': {
        this.need(entry, 'commands');
        const [cid, title] = args;
        entry.commands.set(cid, title);
        CMD['plugin:' + id + ':' + cid] = { label: `${entry.manifest.name || id} : ${title}`, run: () => entry.worker.postMessage({ type: 'command', id: cid }) };
        return true;
      }
      case 'commands.execute': {
        this.need(entry, 'commands');
        // Commands of the Studio that only act on the open file or the views.
        const allowed = ['save', 'saveAll', 'check', 'run', 'test', 'build', 'formatDocument', 'gotoLine', 'find', 'showProblems', 'toggleTerminal', 'showOutline', 'showTests'];
        const c = String(args[0]);
        if (!CMD[c] || !(allowed.includes(c) || c.startsWith('plugin:'))) throw new Error('commande non autorisée : ' + c);
        run(c);
        return true;
      }
      case 'window.showMessage': this.need(entry, 'notifications'); toast(`${entry.manifest.name || id} : ${args[0]}`, ['info', 'success', 'error'].includes(args[1]) ? args[1] : 'info'); return true;
      case 'window.prompt': this.need(entry, 'notifications'); return promptBox(`${entry.manifest.name || id}`, { label: args[0], value: args[1] });
      case 'window.setStatus': {
        this.need(entry, 'statusbar');
        let el = $(`#sb-plugin-${CSS.escape(id)}`);
        if (!el) { el = h('div', { class: 'sb-item', id: `sb-plugin-${id}`, title: entry.manifest.name || id }); $('.sb-spacer').after(el); }
        el.textContent = String(args[0]).slice(0, 80);
        return true;
      }
      case 'editor.getText': this.need(entry, 'editor.read'); return ed().value;
      case 'editor.getSelection': this.need(entry, 'editor.read'); { const e = ed(); return e.value.slice(e.ta.selectionStart, e.ta.selectionEnd); }
      case 'editor.getPath': this.need(entry, 'editor.read'); return S.active ? S.active.path : null;
      case 'editor.getLanguage': this.need(entry, 'editor.read'); return S.active && S.active.ed ? S.active.ed.lang : null;
      case 'editor.replaceSelection': {
        this.need(entry, 'editor.write');
        const e = ed();
        if (e.ta.readOnly) throw new Error('fichier en lecture seule');
        const a = e.ta.selectionStart, b = e.ta.selectionEnd, t = String(args[0]);
        e.replace(a, b, t, a + t.length);
        return true;
      }
      case 'editor.setText': {
        this.need(entry, 'editor.write');
        const e = ed();
        if (e.ta.readOnly) throw new Error('fichier en lecture seule');
        e.replace(0, e.value.length, String(args[0]), Math.min(e.ta.selectionStart, String(args[0]).length));
        return true;
      }
      case 'workspace.listFiles': this.need(entry, 'workspace.read'); return [...S.files];
      case 'workspace.readFile': this.need(entry, 'workspace.read'); return textOf(safePath(args[0]));
      case 'workspace.writeFile': {
        this.need(entry, 'workspace.write');
        const p = safePath(args[0]);
        const open = S.docs.find((d) => d.path === p && d.ed);
        if (open) { open.ed.replace(0, open.ed.value.length, String(args[1]), 0); await saveDoc(open, true); }
        else { await api.post('file?' + qs({ path: p }), String(args[1])); refreshTree(); }
        return true;
      }
      case 'compiler.check': {
        this.need(entry, 'compiler');
        const r = await api.post('check?' + qs({ path: safePath(args[0]) }));
        return r.diagnostics;
      }
      case 'compiler.format': {
        this.need(entry, 'compiler');
        const doc = S.docs.find((d) => d.ed && extOf(d.path) === 'nx' && !d.virtual) || { path: S.files.find((f) => extOf(f) === 'nx') };
        if (!doc.path) throw new Error('aucun fichier NEXUL dans le dossier');
        return api.req('POST', 'ide?' + qs({ req: 'format', path: doc.path }), String(args[0]));
      }
      default: throw new Error('fonction inconnue : ' + method);
    }
  },

  /* Events sent to the extensions that may read the editor. */
  emit(name, data) {
    for (const [, e] of this.running) if (e.perms.has('editor.read')) e.worker.postMessage({ type: 'event', name, data });
  },

  async toggle(p) {
    if (this.running.has(p.id) || this.enabledOf(p.id)) {
      this.stop(p.id);
      this.setEnabled(p.id, false);
      this.render();
      return;
    }
    const perms = p.manifest.permissions || [];
    const unknown = perms.filter((x) => !PERMISSIONS[x]);
    if (unknown.length) { toast(`Permissions inconnues : ${unknown.join(', ')}`, 'error'); return; }
    const text = perms.length
      ? `L'extension « ${p.manifest.name || p.id} » demande :\n\n` + perms.map((x) => '• ' + PERMISSIONS[x]).join('\n') + '\n\nElle s\'exécute isolée et ne peut rien faire d\'autre.'
      : `L'extension « ${p.manifest.name || p.id} » ne demande aucune permission.`;
    if (!(await confirmBox('Activer l\'extension ?', text, 'Activer'))) return;
    this.setEnabled(p.id, true, perms);
    await this.start(p);
    this.render();
  },

  render() {
    const el = $('.sb-view[data-view="plugins"]');
    if (!el || S.sidebarView !== 'plugins') return;
    el.innerHTML = '';
    el.append(h('div', { class: 'sb-head' }, h('span', { text: 'EXTENSIONS' }), h('div', { class: 'sb-actions' },
      h('button', { class: 'icon-btn', title: 'Ouvrir le dossier des extensions', html: icon('folderOpen'), onclick: () => api.post('plugins/open-folder') }),
      h('button', { class: 'icon-btn', title: 'Recharger les extensions', html: icon('refresh'), onclick: async () => { for (const id of [...this.running.keys()]) this.stop(id); await this.load(); toast('Extensions rechargées.', 'success', 1500); } }))));
    const box = h('div', { class: 'sb-list' });
    el.append(box);
    if (!this.list.length) box.append(h('div', { class: 'sb-empty', html: 'Aucune extension.<br><br>Une extension est un dossier contenant <code>plugin.json</code> et un script, placé dans le dossier des extensions. Voir <code>docs/studio/extensions.md</code>.' }));
    for (const p of this.list) {
      const m = p.manifest;
      const on = this.running.has(p.id);
      const st = this.status.get(p.id);
      const sw = h('div', { class: 'switch' + (on ? ' on' : ''), title: on ? 'Désactiver' : 'Activer', onclick: () => this.toggle(p) });
      box.append(h('div', { class: 'plugin-card' },
        h('h4', {}, iconEl('puzzle'), h('span', { text: m.name || p.id }), h('span', { class: 'ver', text: 'v' + (m.version || '?') }), sw),
        h('p', { text: m.description || '' }),
        h('div', { class: 'plugin-perms' }, ...(m.permissions || []).map((x) => h('span', { class: 'plugin-perm', title: PERMISSIONS[x] || 'inconnue', text: x }))),
        h('p', { style: { margin: 0, color: 'var(--fg-faint)' }, text: [m.author ? 'par ' + m.author : '', p.builtin ? 'fournie avec NEXUL Studio' : 'installée par vous', st && st !== 'active' ? st : ''].filter(Boolean).join(' · ') })));
    }
  },
};
window.PLUGINS = PLUGINS;

/* Events for the extensions. */
(function hookEvents() {
  const _save = saveDoc;
  saveDoc = async function (doc, manual) {
    const ok = await _save(doc, manual);
    if (ok && doc && doc.ed) PLUGINS.emit('save', { path: doc.path });
    return ok;
  };
  const _act = activate;
  activate = function (doc, keepFocus) {
    _act(doc, keepFocus);
    if (doc) PLUGINS.emit('activate', { path: doc.path });
  };
  const _chg = editorHooks.change;
  editorHooks.change = function (ed) {
    _chg(ed);
    clearTimeout(PLUGINS.changeTimer);
    PLUGINS.changeTimer = setTimeout(() => { const d = S.docs.find((x) => x.ed === ed); if (d) PLUGINS.emit('change', { path: d.path }); }, 400);
  };
  MENUS.splice(MENUS.length - 1, 0, ['Extensions', ['showPlugins']]);
  const _bl = buildLayout;
  buildLayout = function () {
    _bl();
    PLUGINS.load().then(() => {
      // The commands of the extensions in their menu.
      const menu = MENUS.find((x) => x[0] === 'Extensions')[1];
      const sync = () => {
        menu.length = 0;
        menu.push('showPlugins');
        const ids = Object.keys(CMD).filter((k) => k.startsWith('plugin:'));
        if (ids.length) menu.push('-', ...ids);
      };
      sync();
      setInterval(sync, 1500);
    });
  };
})();
