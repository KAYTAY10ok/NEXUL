'use strict';

const fs = require('node:fs');
const path = require('node:path');

function editionFromFileHeader(text) {
  for (const line of String(text || '').split(/\r?\n/, 3)) {
    const match = /^\/\/ edition:\s*(0\.2|1)\s*$/.exec(line.trim());
    if (match) return match[1];
  }
  return undefined;
}

function editionFromManifest(text) {
  let section = '';
  for (const rawLine of String(text || '').split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;
    const header = /^\[([^\]]+)\]\s*(?:#.*)?$/.exec(line);
    if (header) {
      section = header[1].trim();
      continue;
    }
    if (section !== 'package') continue;
    const edition = /^edition\s*=\s*(["'])(0\.2|1)\1$/.exec(line);
    if (edition) return edition[2];
  }
  return undefined;
}

function editionForFile(documentText, filePath) {
  const declared = editionFromFileHeader(documentText);
  if (declared) return declared;

  let directory = path.dirname(filePath);
  while (true) {
    const manifest = path.join(directory, 'nexul.toml');
    try {
      const contents = fs.readFileSync(manifest, 'utf8');
      return editionFromManifest(contents) || '0.2';
    } catch (error) {
      if (error.code !== 'ENOENT' && error.code !== 'ENOTDIR') throw error;
    }
    const parent = path.dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  return '0.2';
}

module.exports = {
  editionForFile,
  editionFromFileHeader,
  editionFromManifest
};
