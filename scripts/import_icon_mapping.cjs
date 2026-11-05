const assert = require('node:assert/strict');
const fs = require('node:fs');
const { stripTypeScriptTypes } = require('node:module');
const path = require('node:path');
const vm = require('node:vm');

assert.equal(process.argv.length, 3, 'Usage: node scripts/import_icon_mapping.cjs <upstream-checkout>');
const upstream = path.resolve(process.argv[2], 'src/core');
const mappingPath = path.resolve(__dirname, '../assets/material/mapping.json');
const selected = new Set(JSON.parse(fs.readFileSync(mappingPath, 'utf8')).map(icon => icon.name));
const context = vm.createContext({});
for (const module of [
  'models/icons/iconPack.ts',
  'models/icons/patterns/patterns.ts',
  'patterns/patterns.ts',
  'icons/fileIcons.ts',
  'generator/config/defaultConfig.ts',
]) {
  const source = stripTypeScriptTypes(fs.readFileSync(path.join(upstream, module), 'utf8'), {
    mode: 'transform',
  }).replace(/^import[\s\S]*?;\n/gm, '').replace(/\bexport /g, '');
  vm.runInContext(source, context, { filename: module, timeout: 5000 });
}
const definitions = vm.runInContext('fileIcons', context);
const pack = vm.runInContext('getDefaultConfig().activeIconPack', context);
const enabled = definitions.icons.filter(icon =>
  !icon.disabled && (!icon.enabledFor || icon.enabledFor.includes(pack)));
const filenames = new Map();
const extensions = new Map();
for (const icon of enabled) {
  for (const name of icon.fileNames || []) filenames.set(name.toLowerCase(), icon.name);
  for (const extension of icon.fileExtensions || []) extensions.set(extension.toLowerCase(), icon.name);
}
const fallback = definitions.defaultIcon.name;
assert(selected.delete(fallback), 'Selection must include the upstream fallback');
const mappings = [{ extensions: [], filenames: [], name: fallback }];
for (const icon of enabled) {
  if (!selected.delete(icon.name)) continue;
  assert(!icon.clone, `Selected icon requires upstream clone generation: ${icon.name}`);
  mappings.push({
    extensions: [...extensions].filter(([, name]) => name === icon.name).map(([key]) => key),
    filenames: [...filenames].filter(([, name]) => name === icon.name).map(([key]) => key),
    name: icon.name,
  });
}
assert.equal(selected.size, 0, `Selected icons missing upstream: ${[...selected].join(', ')}`);
fs.writeFileSync(mappingPath, JSON.stringify(mappings, null, 2) + '\n');
