// Extension d'exemple : des statistiques sur le fichier ouvert, mises à jour à chaque
// modification.
async function update() {
  let text;
  try { text = await nexul.editor.getText(); } catch (e) { await nexul.window.setStatus(''); return; }
  const lines = text.split('\n').length;
  const fns = (text.match(/^\s*(pub\s+)?fn\s+\w+/gm) || []).length;
  await nexul.window.setStatus(`${lines} lignes · ${fns} fonction${fns > 1 ? 's' : ''}`);
}

nexul.events.on('activate', update);
nexul.events.on('change', update);
update();
