'use strict';
/* NEXUL Studio — languages: detection, line tokenizers, completion data.
 *
 * A tokenizer takes one line and the state at its start, and returns
 * { toks: [[start, end, cls, extra?], ...], st: stateAtEnd }.
 * States are small arrays so that they can be compared cheaply (join).
 */

const LANGS = {
  nexul: { name: 'NEXUL', comment: '//', pairs: '()[]{}""\'\'' },
  html: { name: 'HTML', comment: ['<!--', '-->'], pairs: '()[]{}""' },
  css: { name: 'CSS', comment: ['/*', '*/'], pairs: '()[]{}""\'\'' },
  js: { name: 'JavaScript', comment: '//', pairs: '()[]{}""\'\'``' },
  json: { name: 'JSON', comment: null, pairs: '[]{}""' },
  md: { name: 'Markdown', comment: ['<!--', '-->'], pairs: '()[]' },
  text: { name: 'Texte brut', comment: null, pairs: '()[]{}' },
};

function langOf(path) {
  const ext = (path.split('.').pop() || '').toLowerCase();
  if (ext === 'nx') return 'nexul';
  if (ext === 'html' || ext === 'htm') return 'html';
  if (ext === 'css') return 'css';
  if (ext === 'js' || ext === 'mjs' || ext === 'cjs') return 'js';
  if (ext === 'json') return 'json';
  if (ext === 'md' || ext === 'markdown') return 'md';
  return 'text';
}

const isDigit = (c) => c >= '0' && c <= '9';
const isIdentStart = (c) => !!c && ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c === '_');
const isIdent = (c) => !!c && (isIdentStart(c) || isDigit(c));
const scanIdent = (line, i) => { while (i < line.length && isIdent(line[i])) i++; return i; };
const OPS = '+-*/%=<>!&|^~?:.';

function scanNumber(line, i) {
  const n = line.length;
  if (line[i] === '0' && (line[i + 1] === 'x' || line[i + 1] === 'b' || line[i + 1] === 'o')) {
    i += 2;
    while (i < n && /[0-9a-fA-F_]/.test(line[i])) i++;
    return i;
  }
  while (i < n && (isDigit(line[i]) || line[i] === '_')) i++;
  if (line[i] === '.' && isDigit(line[i + 1])) {
    i++;
    while (i < n && (isDigit(line[i]) || line[i] === '_')) i++;
  }
  if ((line[i] === 'e' || line[i] === 'E') && (isDigit(line[i + 1]) || ((line[i + 1] === '+' || line[i + 1] === '-') && isDigit(line[i + 2])))) {
    i += 2;
    while (i < n && isDigit(line[i])) i++;
  }
  return i;
}

/* ------------------------------------------------------------------ NEXUL */

const NX_KW = new Set(['and', 'as', 'const', 'dyn', 'enum', 'extern', 'fn', 'impl', 'import', 'in', 'indirect', 'inout', 'let', 'module', 'move', 'not', 'or', 'owned', 'pub', 'static', 'struct', 'trait', 'type', 'unsafe', 'var', 'where',
  'async', 'await', 'class', 'defer', 'interface', 'loop', 'macro', 'ref', 'try', 'yield']);
const NX_CTL = new Set(['if', 'else', 'match', 'for', 'while', 'return', 'break', 'continue']);
const NX_TYPES = new Set(['i8', 'i16', 'i32', 'i64', 'isize', 'u8', 'u16', 'u32', 'u64', 'usize', 'f32', 'f64', 'bool', 'char', 'Never', 'Ptr']);

// Finds the `}` closing an interpolation that starts after `i` (skipping nested strings).
function nxInterpEnd(line, i) {
  let depth = 0;
  const n = line.length;
  while (i < n) {
    const c = line[i];
    if (c === '"') {
      i = nxStringEnd(line, i, false);
      continue;
    }
    if (c === '\'') {
      i++;
      while (i < n && line[i] !== '\'') i += line[i] === '\\' ? 2 : 1;
      i++;
      continue;
    }
    if (c === '(' || c === '[' || c === '{') depth++;
    else if (c === ')' || c === ']') depth--;
    else if (c === '}') {
      if (depth === 0) return i;
      depth--;
    }
    i++;
  }
  return -1;
}

function nxStringEnd(line, i, raw) {
  const n = line.length;
  i++;
  while (i < n) {
    const c = line[i];
    if (c === '"') return i + 1;
    if (c === '\\' && !raw) { i += 2; continue; }
    if (c === '{' && !raw) {
      const e = nxInterpEnd(line, i + 1);
      if (e < 0) return n;
      i = e + 1;
      continue;
    }
    i++;
  }
  return n;
}

function nxString(line, i, toks, raw, d) {
  const n = line.length;
  const start = i;
  i += raw ? 2 : 1;
  let seg = start;
  const flush = (to) => { if (to > seg) toks.push([seg, to, 'str']); };
  while (i < n) {
    const c = line[i];
    if (c === '"') { i++; flush(i); return i; }
    if (c === '\\' && !raw) {
      flush(i);
      let j = i + 2;
      if (line[i + 1] === 'u' && line[i + 2] === '{') { const e = line.indexOf('}', i); j = e < 0 ? n : e + 1; }
      toks.push([i, Math.min(j, n), 'esc']);
      i = j; seg = i;
      continue;
    }
    if (c === '{' && !raw) {
      flush(i);
      const e = nxInterpEnd(line, i + 1);
      if (e < 0) { toks.push([i, n, 'str']); return n; }
      toks.push([i, i + 1, 'interp']);
      // The expression, and an optional format spec after a top-level `:`.
      const inner = line.slice(i + 1, e);
      let colon = -1, depth = 0;
      for (let k = 0; k < inner.length; k++) {
        const ch = inner[k];
        if (ch === '"') { k = nxStringEnd(inner, k, false) - 1; continue; }
        if ('([{'.includes(ch)) depth++;
        else if (')]}'.includes(ch)) depth--;
        else if (ch === ':' && depth === 0) { colon = k; break; }
      }
      const exprEnd = colon < 0 ? inner.length : colon;
      const sub = tokNexul(inner.slice(0, exprEnd), [0, d]);
      for (const t of sub.toks) toks.push([t[0] + i + 1, t[1] + i + 1, t[2]]);
      if (colon >= 0) toks.push([i + 1 + colon, e, 'interp']);
      toks.push([e, e + 1, 'interp']);
      i = e + 1; seg = i;
      continue;
    }
    i++;
  }
  flush(n);
  return n;
}

function tokNexul(line, st) {
  let c = st[0], d = st[1];
  const toks = [];
  const n = line.length;
  let i = 0, prev = '';
  const blockComment = (from) => {
    let j = from;
    while (j < n && c > 0) {
      if (line.startsWith('/*', j)) { c++; j += 2; } else if (line.startsWith('*/', j)) { c--; j += 2; } else j++;
    }
    return j;
  };
  while (i < n) {
    if (c > 0) { const j = blockComment(i); toks.push([i, j, 'comment']); i = j; continue; }
    const ch = line[i];
    if (ch === ' ' || ch === '\t') { i++; continue; }
    if (ch === '/' && line[i + 1] === '/') { toks.push([i, n, 'comment']); break; }
    if (ch === '/' && line[i + 1] === '*') { c = 1; const j = blockComment(i + 2); toks.push([i, j, 'comment']); i = j; continue; }
    if (ch === '"') { i = nxString(line, i, toks, false, d); prev = ''; continue; }
    if (ch === 'r' && line[i + 1] === '"' && !isIdent(line[i - 1])) { i = nxString(line, i, toks, true, d); prev = ''; continue; }
    if (ch === '\'') {
      let j = i + 1;
      if (line[j] === '\\') { j = line[j + 1] === 'u' ? (line.indexOf('}', j) + 1 || n) : j + 2; }
      else { const cp = line.codePointAt(j); j += cp > 0xffff ? 2 : 1; }
      if (line[j] === '\'') j++;
      toks.push([i, Math.min(j, n), 'str']); i = Math.min(j, n); prev = ''; continue;
    }
    if (isDigit(ch)) { const j = scanNumber(line, i); toks.push([i, j, 'num']); i = j; prev = ''; continue; }
    if (ch === '@' && isIdentStart(line[i + 1])) { const j = scanIdent(line, i + 1); toks.push([i, j, 'attr']); i = j; continue; }
    if (isIdentStart(ch)) {
      const j = scanIdent(line, i);
      const w = line.slice(i, j);
      let cls;
      if (NX_CTL.has(w)) cls = 'ctl';
      else if (NX_KW.has(w)) cls = 'kw';
      else if (w === 'self' || w === 'Self') cls = 'self';
      else if (w === 'true' || w === 'false') cls = 'const';
      else if (prev === 'fn') cls = 'fndef';
      else if (prev === 'struct' || prev === 'enum' || prev === 'type') cls = 'type';
      else if (NX_TYPES.has(w)) cls = 'type';
      else if (/^[A-Z][A-Z0-9_]+$/.test(w)) cls = 'const';
      else if (/^[A-Z]/.test(w)) cls = 'type';
      else {
        let k = j;
        while (line[k] === ' ') k++;
        cls = line[k] === '(' ? 'fn' : 'ident';
      }
      toks.push([i, j, cls]);
      prev = w; i = j;
      continue;
    }
    if (ch === '(' || ch === '[' || ch === '{') { toks.push([i, i + 1, 'b' + (d % 3)]); d++; i++; continue; }
    if (ch === ')' || ch === ']' || ch === '}') { d = Math.max(0, d - 1); toks.push([i, i + 1, 'b' + (d % 3)]); i++; continue; }
    if (OPS.includes(ch)) {
      let j = i;
      while (j < n && OPS.includes(line[j]) && !line.startsWith('//', j) && !line.startsWith('/*', j)) j++;
      toks.push([i, j, 'op']); i = j; continue;
    }
    if (ch === ',' || ch === ';') { toks.push([i, i + 1, 'punct']); i++; continue; }
    i++;
  }
  return { toks, st: [c, d] };
}

/* ------------------------------------------------------------------ JavaScript */

const JS_KW = new Set(['const', 'let', 'var', 'function', 'class', 'extends', 'new', 'import', 'export', 'from', 'default', 'async', 'await', 'typeof', 'instanceof', 'in', 'of', 'delete', 'void', 'static', 'get', 'set', 'yield', 'super']);
const JS_CTL = new Set(['if', 'else', 'for', 'while', 'do', 'switch', 'case', 'break', 'continue', 'return', 'try', 'catch', 'finally', 'throw']);
const JS_CONST = new Set(['true', 'false', 'null', 'undefined', 'NaN', 'Infinity']);

// state: [inBlockComment, inTemplate, bracketDepth]
function tokJs(line, st) {
  let c = st[0], tpl = st[1], d = st[2];
  const toks = [];
  const n = line.length;
  let i = 0;
  while (i < n) {
    if (c) {
      const e = line.indexOf('*/', i);
      const j = e < 0 ? n : e + 2;
      toks.push([i, j, 'comment']);
      if (e >= 0) c = 0;
      i = j; continue;
    }
    if (tpl) {
      let j = i;
      while (j < n && line[j] !== '`' && !(line[j] === '$' && line[j + 1] === '{')) j += line[j] === '\\' ? 2 : 1;
      if (j > i) toks.push([i, Math.min(j, n), 'str']);
      if (j >= n) { i = n; break; }
      if (line[j] === '`') { toks.push([j, j + 1, 'str']); tpl = 0; i = j + 1; continue; }
      // ${ ... }
      const e = line.indexOf('}', j);
      const end = e < 0 ? n : e;
      toks.push([j, j + 2, 'interp']);
      const sub = tokJs(line.slice(j + 2, end), [0, 0, d]);
      for (const t of sub.toks) toks.push([t[0] + j + 2, t[1] + j + 2, t[2]]);
      if (e >= 0) toks.push([e, e + 1, 'interp']);
      i = e < 0 ? n : e + 1;
      continue;
    }
    const ch = line[i];
    if (ch === ' ' || ch === '\t') { i++; continue; }
    if (ch === '/' && line[i + 1] === '/') { toks.push([i, n, 'comment']); break; }
    if (ch === '/' && line[i + 1] === '*') { c = 1; toks.push([i, i + 2, 'comment']); i += 2; continue; }
    if (ch === '"' || ch === '\'') {
      let j = i + 1;
      while (j < n && line[j] !== ch) j += line[j] === '\\' ? 2 : 1;
      j = Math.min(j + 1, n);
      toks.push([i, j, 'str']); i = j; continue;
    }
    if (ch === '`') { tpl = 1; toks.push([i, i + 1, 'str']); i++; continue; }
    if (isDigit(ch)) { const j = scanNumber(line, i); toks.push([i, j, 'num']); i = j; continue; }
    if (isIdentStart(ch) || ch === '$') {
      let j = i + 1;
      while (j < n && (isIdent(line[j]) || line[j] === '$')) j++;
      const w = line.slice(i, j);
      let cls = 'ident';
      if (JS_CTL.has(w)) cls = 'ctl';
      else if (JS_KW.has(w)) cls = 'kw';
      else if (JS_CONST.has(w)) cls = 'const';
      else if (w === 'this') cls = 'self';
      else if (/^[A-Z][A-Z0-9_]+$/.test(w)) cls = 'const';
      else if (/^[A-Z]/.test(w)) cls = 'type';
      else {
        let k = j;
        while (line[k] === ' ') k++;
        if (line[k] === '(') cls = 'fn';
        else if (line[i - 1] === '.') cls = 'prop';
      }
      toks.push([i, j, cls]); i = j; continue;
    }
    if ('([{'.includes(ch)) { toks.push([i, i + 1, 'b' + (d % 3)]); d++; i++; continue; }
    if (')]}'.includes(ch)) { d = Math.max(0, d - 1); toks.push([i, i + 1, 'b' + (d % 3)]); i++; continue; }
    if (OPS.includes(ch)) {
      let j = i;
      while (j < n && OPS.includes(line[j]) && !line.startsWith('//', j) && !line.startsWith('/*', j)) j++;
      toks.push([i, j, 'op']); i = j; continue;
    }
    if (ch === ',' || ch === ';') { toks.push([i, i + 1, 'punct']); i++; continue; }
    i++;
  }
  return { toks, st: [c, tpl, d] };
}

/* ------------------------------------------------------------------ CSS */

const HEX_COLOR = /^#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/;

// state: [inComment, braceDepth, inValue]
function tokCss(line, st) {
  let c = st[0], d = st[1], v = st[2];
  const toks = [];
  const n = line.length;
  let i = 0;
  while (i < n) {
    if (c) {
      const e = line.indexOf('*/', i);
      const j = e < 0 ? n : e + 2;
      toks.push([i, j, 'comment']);
      if (e >= 0) c = 0;
      i = j; continue;
    }
    const ch = line[i];
    if (ch === ' ' || ch === '\t') { i++; continue; }
    if (ch === '/' && line[i + 1] === '*') { c = 1; toks.push([i, i + 2, 'comment']); i += 2; continue; }
    if (ch === '{') { toks.push([i, i + 1, 'b' + (d % 3)]); d++; v = 0; i++; continue; }
    if (ch === '}') { d = Math.max(0, d - 1); toks.push([i, i + 1, 'b' + (d % 3)]); v = 0; i++; continue; }
    if (ch === ';') { toks.push([i, i + 1, 'punct']); v = 0; i++; continue; }
    if (ch === '"' || ch === '\'') {
      let j = i + 1;
      while (j < n && line[j] !== ch) j += line[j] === '\\' ? 2 : 1;
      j = Math.min(j + 1, n);
      toks.push([i, j, 'str']); i = j; continue;
    }
    if (ch === '@') { const j = scanIdent(line, i + 1); toks.push([i, Math.max(j, i + 1), 'ctl']); i = Math.max(j, i + 1); continue; }
    if (v) {
      if (ch === '#') {
        const m = HEX_COLOR.exec(line.slice(i));
        if (m) { toks.push([i, i + m[0].length, 'num', m[0]]); i += m[0].length; continue; }
      }
      if (isDigit(ch) || (ch === '.' && isDigit(line[i + 1])) || (ch === '-' && (isDigit(line[i + 1]) || line[i + 1] === '.'))) {
        let j = i + 1;
        while (j < n && (isDigit(line[j]) || line[j] === '.')) j++;
        while (j < n && /[a-z%]/i.test(line[j])) j++;
        toks.push([i, j, 'num']); i = j; continue;
      }
      if (ch === '!' ) { const j = scanIdent(line, i + 1); toks.push([i, j, 'ctl']); i = Math.max(j, i + 1); continue; }
      if (ch === '-' && line[i + 1] === '-') { let j = i + 2; while (j < n && /[\w-]/.test(line[j])) j++; toks.push([i, j, 'prop']); i = j; continue; }
      if (isIdentStart(ch) || ch === '-') {
        let j = i + 1;
        while (j < n && /[\w-]/.test(line[j])) j++;
        const w = line.slice(i, j);
        if (line[j] === '(') toks.push([i, j, 'fn']);
        else {
          const named = NAMED_COLORS[w.toLowerCase()];
          toks.push(named ? [i, j, 'const', named] : [i, j, 'const']);
        }
        i = j; continue;
      }
      if ('()'.includes(ch)) { toks.push([i, i + 1, 'punct']); i++; continue; }
      i++; continue;
    }
    // Selector or property name.
    const rest = line.slice(i);
    const braceAt = rest.indexOf('{'), semiAt = rest.indexOf(';'), colonAt = rest.indexOf(':');
    const isSelector = d === 0 || (braceAt >= 0 && (semiAt < 0 || braceAt < semiAt));
    if (isSelector) {
      let j = i;
      while (j < n && line[j] !== '{' && line[j] !== ',' && !line.startsWith('/*', j)) j++;
      if (line[j] === ',') { toks.push([i, j, 'sel']); toks.push([j, j + 1, 'punct']); i = j + 1; continue; }
      toks.push([i, j, 'sel']); i = j; continue;
    }
    if (colonAt >= 0) {
      let j = i;
      while (j < n && line[j] !== ':') j++;
      toks.push([i, j, 'prop']);
      toks.push([j, j + 1, 'punct']);
      v = 1; i = j + 1; continue;
    }
    let j = i;
    while (j < n && line[j] !== ';' && line[j] !== '}') j++;
    toks.push([i, j, 'prop']); i = j;
  }
  return { toks, st: [c, d, v] };
}

const NAMED_COLORS = {
  white: '#ffffff', black: '#000000', red: '#ff0000', green: '#008000', blue: '#0000ff', yellow: '#ffff00', orange: '#ffa500',
  purple: '#800080', pink: '#ffc0cb', gray: '#808080', grey: '#808080', cyan: '#00ffff', magenta: '#ff00ff', navy: '#000080',
  teal: '#008080', gold: '#ffd700', silver: '#c0c0c0', crimson: '#dc143c', tomato: '#ff6347', coral: '#ff7f50', violet: '#ee82ee',
  indigo: '#4b0082', lime: '#00ff00', brown: '#a52a2a', beige: '#f5f5dc', salmon: '#fa8072', orchid: '#da70d6', turquoise: '#40e0d0',
};

/* ------------------------------------------------------------------ HTML */

// state: [mode, tag, closing, sub...]; modes: 0 text, 1 tag, 2 comment, 3 attr "..." , 4 attr '...', 5 script, 6 style
function tokHtml(line, st) {
  let m = st[0], tag = st[1], closing = st[2];
  let jsSt = st[3] || [0, 0, 0], cssSt = st[4] || [0, 0, 0];
  const toks = [];
  const n = line.length;
  let i = 0;
  while (i < n) {
    if (m === 2) {
      const e = line.indexOf('-->', i);
      const j = e < 0 ? n : e + 3;
      toks.push([i, j, 'comment']);
      if (e >= 0) m = 0;
      i = j; continue;
    }
    if (m === 3 || m === 4) {
      const q = m === 3 ? '"' : '\'';
      const e = line.indexOf(q, i);
      const j = e < 0 ? n : e + 1;
      toks.push([i, j, 'str']);
      if (e >= 0) m = 1;
      i = j; continue;
    }
    if (m === 5 || m === 6) {
      const endTag = m === 5 ? '</script' : '</style';
      const e = line.toLowerCase().indexOf(endTag, i);
      const end = e < 0 ? n : e;
      if (end > i) {
        const sub = m === 5 ? tokJs(line.slice(i, end), jsSt) : tokCss(line.slice(i, end), cssSt);
        for (const t of sub.toks) toks.push([t[0] + i, t[1] + i, t[2], t[3]]);
        if (m === 5) jsSt = sub.st; else cssSt = sub.st;
      }
      if (e < 0) { i = n; break; }
      m = 0; i = e; continue;
    }
    if (m === 1) {
      const ch = line[i];
      if (ch === ' ' || ch === '\t') { i++; continue; }
      if (ch === '>' || (ch === '/' && line[i + 1] === '>')) {
        const len = ch === '>' ? 1 : 2;
        toks.push([i, i + len, 'punct']);
        i += len;
        m = 0;
        if (!closing && len === 1) {
          if (tag === 'script') { m = 5; jsSt = [0, 0, 0]; }
          else if (tag === 'style') { m = 6; cssSt = [0, 0, 0]; }
        }
        continue;
      }
      if (ch === '=') { toks.push([i, i + 1, 'op']); i++; continue; }
      if (ch === '"') { m = 3; continue; }
      if (ch === '\'') { m = 4; continue; }
      let j = i;
      while (j < n && !/[\s=>"'/]/.test(line[j])) j++;
      if (j === i) j = i + 1;
      const prevNonSpace = line.slice(0, i).trimEnd().slice(-1);
      toks.push([i, j, prevNonSpace === '=' ? 'str' : 'attrname']);
      i = j; continue;
    }
    // text
    const ch = line[i];
    if (line.startsWith('<!--', i)) { m = 2; continue; }
    if (ch === '<' && (isIdentStart(line[i + 1]) || (line[i + 1] === '/' && isIdentStart(line[i + 2])) || line[i + 1] === '!')) {
      closing = line[i + 1] === '/';
      const s = closing ? i + 2 : i + 1;
      let j = s;
      if (line[i + 1] === '!') j = i + 2;
      while (j < n && /[\w-]/.test(line[j])) j++;
      toks.push([i, s, 'punct']);
      toks.push([s, j, line[i + 1] === '!' ? 'kw' : 'tag']);
      tag = line.slice(s, j).toLowerCase();
      m = 1; i = j; continue;
    }
    if (ch === '&') {
      const e = line.indexOf(';', i);
      if (e > i && e - i < 12) { toks.push([i, e + 1, 'entity']); i = e + 1; continue; }
    }
    let j = i + 1;
    while (j < n && line[j] !== '<' && line[j] !== '&') j++;
    i = j;
  }
  return { toks, st: [m, tag, closing, jsSt, cssSt] };
}

/* ------------------------------------------------------------------ JSON, Markdown */

function tokJson(line, st) {
  let d = st[0];
  const toks = [];
  const n = line.length;
  let i = 0;
  while (i < n) {
    const ch = line[i];
    if (ch === '"') {
      let j = i + 1;
      while (j < n && line[j] !== '"') j += line[j] === '\\' ? 2 : 1;
      j = Math.min(j + 1, n);
      let k = j;
      while (line[k] === ' ') k++;
      toks.push([i, j, line[k] === ':' ? 'prop' : 'str']); i = j; continue;
    }
    if (isDigit(ch) || ch === '-') { const j = scanNumber(line, ch === '-' ? i + 1 : i); toks.push([i, j, 'num']); i = Math.max(j, i + 1); continue; }
    if (isIdentStart(ch)) { const j = scanIdent(line, i); toks.push([i, j, 'const']); i = j; continue; }
    if (ch === '[' || ch === '{') { toks.push([i, i + 1, 'b' + (d % 3)]); d++; i++; continue; }
    if (ch === ']' || ch === '}') { d = Math.max(0, d - 1); toks.push([i, i + 1, 'b' + (d % 3)]); i++; continue; }
    if (ch === ',' || ch === ':') { toks.push([i, i + 1, 'punct']); i++; continue; }
    i++;
  }
  return { toks, st: [d] };
}

function tokMd(line, st) {
  let fence = st[0];
  const toks = [];
  if (line.trimStart().startsWith('```')) { toks.push([0, line.length, 'str']); return { toks, st: [fence ? 0 : 1] }; }
  if (fence) { if (line.length) toks.push([0, line.length, 'str']); return { toks, st: [1] }; }
  const h = /^(#{1,6})\s/.exec(line);
  if (h) { toks.push([0, line.length, 'heading']); return { toks, st: [0] }; }
  const li = /^(\s*)([-*+]|\d+\.)\s/.exec(line);
  if (li) toks.push([li[1].length, li[1].length + li[2].length, 'kw']);
  if (line.startsWith('>')) toks.push([0, 1, 'kw']);
  const re = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\[[^\]]+\]\([^)]+\))/g;
  let mm;
  while ((mm = re.exec(line))) {
    toks.push([mm.index, mm.index + mm[0].length, mm[1] ? 'str' : mm[2] ? 'bold' : 'fn']);
  }
  toks.sort((a, b) => a[0] - b[0]);
  return { toks, st: [0] };
}

const TOKENIZERS = {
  nexul: { init: [0, 0], fn: tokNexul },
  js: { init: [0, 0, 0], fn: tokJs },
  css: { init: [0, 0, 0], fn: tokCss },
  html: { init: [0, '', false, [0, 0, 0], [0, 0, 0]], fn: tokHtml },
  json: { init: [0], fn: tokJson },
  md: { init: [0], fn: tokMd },
  text: { init: [0], fn: (line, st) => ({ toks: [], st }) },
};

/* ------------------------------------------------------------------ completion data */

const NX_SNIPPETS = [
  { label: 'main', detail: 'fonction principale', body: 'fn main() {\n\t$0\n}' },
  { label: 'fn', detail: 'fonction', body: 'fn ${1:nom}(${2}) {\n\t$0\n}' },
  { label: 'fnr', detail: 'fonction avec résultat', body: 'fn ${1:nom}(${2}) -> ${3:i32} {\n\treturn $0\n}' },
  { label: 'struct', detail: 'structure', body: 'struct ${1:Nom} {\n\t$0\n}' },
  { label: 'enum', detail: 'énumération', body: 'enum ${1:Nom} {\n\t$0\n}' },
  { label: 'impl', detail: 'méthodes', body: 'impl ${1:Nom} {\n\t$0\n}' },
  { label: 'trait', detail: 'trait (interface)', body: 'trait ${1:Nom} {\n\tfn ${2:methode}(self) -> ${3:String}\n}' },
  { label: 'implfor', detail: 'implémenter un trait', body: 'impl ${1:Trait} for ${2:Type} {\n\t$0\n}' },
  { label: 'display', detail: 'impl Display', body: 'impl Display for ${1:Type} {\n\tfn to_string(self) -> String {\n\t\treturn "$0"\n\t}\n}' },
  { label: 'derive', detail: '@derive(...)', body: '@derive(${1:Eq, Clone, Display})' },
  { label: 'closure', detail: 'fonction anonyme', body: 'fn(${1:x}) => $0' },
  { label: 'whilelet', detail: 'while let', body: 'while let Some(${1:x}) = ${2:pile.pop()} {\n\t$0\n}' },
  { label: 'map', detail: 'Map (table de hachage)', body: 'var ${1:table}: Map[${2:String}, ${3:i32}] = Map.new()' },
  { label: 'if', detail: 'condition', body: 'if ${1:condition} {\n\t$0\n}' },
  { label: 'ife', detail: 'si / sinon', body: 'if ${1:condition} {\n\t$0\n} else {\n}' },
  { label: 'iflet', detail: 'if let', body: 'if let Some(${1:x}) = ${2:option} {\n\t$0\n}' },
  { label: 'for', detail: 'boucle sur un intervalle', body: 'for ${1:i} in 0..${2:n} {\n\t$0\n}' },
  { label: 'foreach', detail: 'boucle sur une collection', body: 'for ${1:x} in ${2:liste} {\n\t$0\n}' },
  { label: 'while', detail: 'boucle tant que', body: 'while ${1:condition} {\n\t$0\n}' },
  { label: 'match', detail: 'filtrage', body: 'match ${1:valeur} {\n\t${2:_} => $0\n}' },
  { label: 'println', detail: 'afficher une ligne', body: 'println("${1}")' },
  { label: 'test', detail: 'fonction de test', body: '@test\nfn ${1:nom_du_test}() {\n\tassert($0)\n}' },
  { label: 'vec', detail: 'nouveau vecteur', body: 'var ${1:liste}: Vec[${2:i32}] = Vec.new()' },
];

const NX_WORDS = [
  ...['let', 'var', 'fn', 'return', 'if', 'else', 'match', 'for', 'while', 'in', 'break', 'continue', 'struct', 'enum', 'impl', 'import', 'pub', 'const', 'type',
    'inout', 'owned', 'move', 'unsafe', 'extern', 'indirect', 'and', 'or', 'not', 'as', 'true', 'false', 'self', 'Self', 'trait', 'dyn', 'where', 'static'].map((w) => ({ label: w, kind: 'kw' })),
  ...['i8', 'i16', 'i32', 'i64', 'isize', 'u8', 'u16', 'u32', 'u64', 'usize', 'f32', 'f64', 'bool', 'char', 'String', 'Vec', 'Option', 'Result', 'Ptr', 'Never',
    'Map', 'Set', 'Ordering', 'Eq', 'Ord', 'Clone', 'Display', 'Hash', 'Default']
    .map((w) => ({ label: w, kind: 'type' })),
  ...[['Some', 'Option'], ['None', 'Option'], ['Ok', 'Result'], ['Err', 'Result']].map(([w, d]) => ({ label: w, kind: 'type', detail: d })),
  ...[['println', 'affiche puis passe à la ligne'], ['print', 'affiche'], ['eprintln', 'affiche sur la sortie d\'erreur'], ['panic', 'arrête le programme'],
    ['assert', 'vérifie une condition'], ['unreachable', 'code inaccessible']].map(([w, d]) => ({ label: w, kind: 'fn', detail: d })),
];

const NX_MEMBERS = {
  String: ['len', 'is_empty', 'push', 'push_str', 'clone', 'contains', 'find', 'starts_with', 'ends_with', 'substring', 'trim', 'to_uppercase', 'to_lowercase',
    'split', 'lines', 'replace', 'repeat', 'chars', 'bytes', 'byte_at', 'char_count', 'parse_int', 'parse_float', 'clear', 'as_ptr', 'capacity', 'reserve'],
  Vec: ['push', 'pop', 'len', 'is_empty', 'insert', 'remove', 'swap', 'clear', 'truncate', 'reserve', 'capacity'],
  Option: ['is_some', 'is_none', 'unwrap', 'expect', 'unwrap_or', 'take'],
  Result: ['is_ok', 'is_err', 'unwrap', 'unwrap_err', 'expect', 'unwrap_or', 'ok', 'err'],
  num: ['abs', 'min', 'max', 'pow', 'to_string', 'sqrt', 'floor', 'ceil', 'round', 'trunc', 'sin', 'cos', 'tan', 'exp', 'ln', 'log10', 'log2', 'powf', 'is_nan'],
  std: ['io', 'system', 'time', 'math', 'mem', 'random', 'collections'],
  io: ['read_line', 'read_file', 'write_file', 'append_file', 'flush'],
  system: ['args', 'exit', 'env'],
  time: ['monotonic_ns'],
  math: ['PI', 'E', 'TAU', 'SQRT_2', 'LN_2', 'LN_10'],
  mem: ['size_of', 'swap', 'replace', 'drop', 'forget'],
};

const HTML_TAGS = ['div', 'span', 'p', 'a', 'img', 'ul', 'ol', 'li', 'h1', 'h2', 'h3', 'h4', 'section', 'header', 'footer', 'nav', 'main', 'article', 'aside',
  'button', 'input', 'form', 'label', 'textarea', 'select', 'option', 'table', 'thead', 'tbody', 'tr', 'th', 'td', 'strong', 'em', 'small', 'code', 'pre',
  'blockquote', 'figure', 'figcaption', 'video', 'audio', 'canvas', 'svg', 'script', 'style', 'link', 'meta', 'title', 'br', 'hr', 'iframe'];
const VOID_TAGS = new Set(['area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'source', 'track', 'wbr']);
const HTML_ATTRS = ['class', 'id', 'href', 'src', 'alt', 'style', 'type', 'name', 'value', 'placeholder', 'title', 'target', 'rel', 'width', 'height', 'onclick', 'disabled', 'for', 'lang', 'charset', 'content'];

const CSS_PROPS = ['display', 'position', 'top', 'right', 'bottom', 'left', 'width', 'height', 'min-width', 'max-width', 'min-height', 'max-height', 'margin', 'padding',
  'border', 'border-radius', 'background', 'background-color', 'background-image', 'color', 'font-family', 'font-size', 'font-weight', 'line-height', 'letter-spacing',
  'text-align', 'text-decoration', 'text-transform', 'flex', 'flex-direction', 'flex-wrap', 'justify-content', 'align-items', 'gap', 'grid-template-columns',
  'grid-template-rows', 'overflow', 'opacity', 'box-shadow', 'transform', 'transition', 'animation', 'cursor', 'z-index', 'object-fit', 'filter', 'backdrop-filter'];

const JS_WORDS = ['const', 'let', 'function', 'return', 'if', 'else', 'for', 'while', 'class', 'new', 'async', 'await', 'document', 'window', 'console', 'querySelector',
  'querySelectorAll', 'addEventListener', 'getElementById', 'classList', 'textContent', 'innerHTML', 'fetch', 'setTimeout', 'setInterval', 'JSON', 'Math', 'Array', 'Object'];
const JS_SNIPPETS = [
  { label: 'log', detail: 'console.log', body: 'console.log(${1});' },
  { label: 'func', detail: 'fonction', body: 'function ${1:nom}(${2}) {\n\t$0\n}' },
  { label: 'click', detail: 'au clic', body: '${1:element}.addEventListener(\'click\', () => {\n\t$0\n});' },
  { label: 'qs', detail: 'querySelector', body: 'document.querySelector(\'${1}\')' },
];

const HTML_SKELETON = '<!DOCTYPE html>\n<html lang="fr">\n<head>\n\t<meta charset="UTF-8">\n\t<meta name="viewport" content="width=device-width, initial-scale=1.0">\n\t<title>${1:Mon site}</title>\n\t<link rel="stylesheet" href="style.css">\n</head>\n<body>\n\t$0\n\t<script src="script.js"></script>\n</body>\n</html>';
