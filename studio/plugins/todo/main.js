// Extension d'exemple : rassemble les TODO et FIXME des fichiers .nx dans TODO.md.
nexul.commands.register('lister', 'Lister les TODO dans TODO.md', async () => {
  const files = (await nexul.workspace.listFiles()).filter((f) => f.endsWith('.nx'));
  const items = [];
  for (const f of files) {
    const text = await nexul.workspace.readFile(f);
    text.split('\n').forEach((line, i) => {
      const m = /\/\/\s*(TODO|FIXME)\s*:?\s*(.*)$/.exec(line);
      if (m) items.push(`- [ ] **${m[1]}** ${m[2]} — \`${f}:${i + 1}\``);
    });
  }
  const md = `# À faire\n\n${items.length ? items.join('\n') : '_Aucun TODO trouvé._'}\n`;
  await nexul.workspace.writeFile('TODO.md', md);
  await nexul.window.showMessage(`${items.length} élément(s) écrit(s) dans TODO.md.`, 'success');
});
