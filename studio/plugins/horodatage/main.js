// Extension d'exemple : une commande qui insère la date et l'heure.
const pad = (n) => String(n).padStart(2, '0');

nexul.commands.register('inserer', 'Insérer la date et l\'heure', async () => {
  const d = new Date();
  const text = `${pad(d.getDate())}/${pad(d.getMonth() + 1)}/${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  await nexul.editor.insert(text);
});
