import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { ts } from './typescript.mjs';

export const LANGUAGES = ['zh', 'en'];
export const SOURCE = 'zh';
const pluralPattern = /_(zero|one|two|few|many|other)$/;
const sorted = values => [...values].sort();
export const json = value => JSON.stringify(value, null, 2) + '\n';
export const hash = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const own = (value, key) => Object.prototype.hasOwnProperty.call(value, key);

/** JSON.parse 会静默覆盖重复键；先使用 TypeScript JSON AST 检查重复成员。 */
export function readJsonStrictText(text, filename = 'catalog.json') {
  const tree = ts.parseJsonText(filename, text);
  if (tree.parseDiagnostics.length) throw new Error(`${filename}: invalid JSON syntax`);
  const errors = [];
  function visit(node) {
    if (ts.isObjectLiteralExpression(node)) {
      const names = new Set();
      for (const member of node.properties) {
        const name = member.name?.text;
        if (typeof name !== 'string') { errors.push('non-literal member'); continue; }
        if (names.has(name)) errors.push(`duplicate member ${JSON.stringify(name)}`);
        if (['__proto__', 'constructor', 'prototype'].includes(name)) errors.push(`reserved member ${name}`);
        names.add(name);
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(tree);
  if (errors.length) throw new Error(`${filename}: ${errors.join('; ')}`);
  return JSON.parse(text);
}
export function readJson(file) { return readJsonStrictText(fs.readFileSync(file, 'utf8'), file); }

export function placeholders(message) {
  if (typeof message !== 'string' || !message.trim()) throw new Error('message is empty or is not a string');
  if (/<\/?[a-z][^>]*>/i.test(message) || /\$t\s*\(/.test(message)) throw new Error('HTML and nested translations are not supported');
  const names = new Set();
  const remainder = message.replace(/{{\s*([A-Za-z][A-Za-z0-9]*)\s*}}/g, (_, name) => { names.add(name); return ''; });
  if (/[{}]/.test(remainder)) throw new Error('use simple {{name}} placeholders; ICU, unescaped placeholders and inline formatting are not supported');
  return sorted(names);
}

export function inspectNamespace(data, locale, namespace, { allowEmpty = false } = {}) {
  if (!data || Array.isArray(data) || typeof data !== 'object') throw new Error(`${locale}/${namespace}: expected a flat object`);
  const groups = new Map();
  const issues = [];
  for (const key of Object.keys(data).sort()) {
    if (!/^[A-Za-z][A-Za-z0-9]*(?:\.[A-Za-z][A-Za-z0-9]*)*(?:_(zero|one|two|few|many|other))?$/.test(key)) {
      issues.push(`${locale}/${namespace}:${key}: invalid key`); continue;
    }
    const form = key.match(pluralPattern)?.[1] ?? null;
    const base = form ? key.replace(pluralPattern, '') : key;
    let group = groups.get(base);
    if (!group) { group = { id: `${namespace}:${base}`, base, plural: form !== null, values: {}, params: null }; groups.set(base, group); }
    if (group.plural !== (form !== null)) issues.push(`${locale}/${namespace}:${base}: plural and non-plural key collide`);
    group.values[form ?? 'text'] = data[key];
    try {
      const params = allowEmpty && data[key] === '' ? null : placeholders(data[key]);
      if (params) {
        if (group.params && JSON.stringify(params) !== JSON.stringify(group.params)) issues.push(`${locale}/${namespace}:${key}: plural variants use different parameters`);
        group.params = params;
        if (form && !params.includes('count')) issues.push(`${locale}/${namespace}:${key}: plural message must contain {{count}}`);
      }
    } catch (error) { issues.push(`${locale}/${namespace}:${key}: ${error.message}`); }
  }
  for (const group of groups.values()) {
    if (group.plural) {
      const required = new Intl.PluralRules(locale === 'zh' ? 'zh-CN' : locale).resolvedOptions().pluralCategories;
      for (const form of required) if (!own(group.values, form)) issues.push(`${group.id}/${locale}: missing plural form ${form}`);
      for (const form of Object.keys(group.values)) if (!required.includes(form) && form !== 'zero') issues.push(`${group.id}/${locale}: unused plural form ${form}`);
    }
    group.params ??= [];
  }
  if (issues.length) throw new Error(issues.join('\n'));
  return groups;
}

export function loadCatalog(root, options = {}) {
  const localesDir = path.join(root, 'locales');
  const namespaces = fs.readdirSync(path.join(localesDir, SOURCE)).filter(n => n.endsWith('.json')).map(n => n.slice(0, -5)).sort();
  if (!namespaces.length) throw new Error('Source catalog is empty');
  const resources = {}, groups = {};
  for (const locale of LANGUAGES) {
    const files = fs.readdirSync(path.join(localesDir, locale)).filter(n => n.endsWith('.json')).map(n => n.slice(0, -5)).sort();
    if (JSON.stringify(files) !== JSON.stringify(namespaces)) throw new Error(`${locale}: namespace files differ from source`);
    resources[locale] = {}; groups[locale] = new Map();
    for (const namespace of namespaces) {
      if (!/^[a-z][a-z0-9-]*$/.test(namespace)) throw new Error(`Invalid namespace ${namespace}`);
      const values = readJson(path.join(localesDir, locale, `${namespace}.json`));
      resources[locale][namespace] = values;
      for (const [base, group] of inspectNamespace(values, locale, namespace, options)) groups[locale].set(`${namespace}:${base}`, group);
    }
  }
  return { root, namespaces, resources, groups };
}

function stableValues(group) { return Object.fromEntries(Object.entries(group.values).sort(([a], [b]) => a.localeCompare(b, 'en'))); }
export function sourceHash(group, context) { return hash({ values: stableValues(group), context }); }
export function targetHash(group) { return hash(stableValues(group)); }
export function loadContext(root) { return readJson(path.join(root, 'context.json')); }
export function structuralIssues(catalog) {
  const errors = [];
  const source = catalog.groups[SOURCE];
  for (const locale of LANGUAGES.filter(l => l !== SOURCE)) {
    for (const [id, group] of source) {
      const target = catalog.groups[locale].get(id);
      if (!target) { errors.push(`${locale}: missing key ${id}`); continue; }
      if (group.plural !== target.plural) errors.push(`${locale}/${id}: plural behavior differs`);
      if (JSON.stringify(group.params) !== JSON.stringify(target.params)) errors.push(`${locale}/${id}: placeholder names differ`);
    }
    for (const id of catalog.groups[locale].keys()) if (!source.has(id)) errors.push(`${locale}: obsolete key ${id}`);
  }
  return errors;
}
export function checkCatalog(root, { metadata = true, generated = true } = {}) {
  const catalog = loadCatalog(root);
  const errors = structuralIssues(catalog);
  const context = loadContext(root);
  for (const id of catalog.groups[SOURCE].keys()) if (typeof context[id] !== 'string' || !context[id].trim()) errors.push(`${id}: missing translator context`);
  for (const id of Object.keys(context)) if (!catalog.groups[SOURCE].has(id)) errors.push(`${id}: obsolete translator context`);
  if (metadata) {
    for (const locale of LANGUAGES.filter(l => l !== SOURCE)) {
      const records = readJson(path.join(root, 'metadata', `${locale}.json`));
      for (const [id, source] of catalog.groups[SOURCE]) {
        const target = catalog.groups[locale].get(id), record = records[id];
        if (!record || !['translated', 'reviewed'].includes(record.state)) { errors.push(`${locale}/${id}: translation not accepted`); continue; }
        if (!['model-authored','model-assisted','human','imported'].includes(record.origin) || !record.note?.trim()) errors.push(`${locale}/${id}: missing provenance`);
        if (record.state === 'reviewed' && record.origin !== 'human') errors.push(`${locale}/${id}: reviewed requires explicit human provenance`);
        if (record.sourceHash !== sourceHash(source, context[id])) errors.push(`${locale}/${id}: source changed; target needs review`);
        if (target && record.targetHash !== targetHash(target)) errors.push(`${locale}/${id}: target changed without acceptance`);
      }
      for (const id of Object.keys(records)) if (!catalog.groups[SOURCE].has(id)) errors.push(`${locale}/${id}: obsolete metadata`);
    }
  }
  if (generated) for (const [name, content] of Object.entries(generatedFiles(catalog))) {
    if (!fs.existsSync(path.join(root, name)) || fs.readFileSync(path.join(root, name), 'utf8') !== content) errors.push(`${name}: generated output is stale; run i18n:generate`);
  }
  return { catalog, errors, logicalKeys: catalog.groups[SOURCE].size, physicalKeys: Object.fromEntries(LANGUAGES.map(l => [l, Object.values(catalog.resources[l]).reduce((n, ns) => n + Object.keys(ns).length, 0)])) };
}

export function generatedFiles(catalog) {
  const entries = [...catalog.groups[SOURCE]].sort(([a], [b]) => a.localeCompare(b, 'en'));
  const modern = entries.filter(([id]) => !id.startsWith('legacy:'));
  let types = '// 由 build/i18n/cli.mjs 生成，请勿手工编辑。\nexport interface MessageArguments {\n';
  for (const [id, g] of modern) {
    const parameters = g.params.length === 0 ? 'Record<never, never>' : `{ ${g.params.map(p => `readonly ${p}: ${p === 'count' ? 'number' : 'string'}`).join('; ')} }`;
    types += `  ${JSON.stringify(id)}: ${parameters};\n`;
  }
  types += '}\nexport type MessageKey = keyof MessageArguments;\nexport type PlainMessageKey = { [K in MessageKey]: keyof MessageArguments[K] extends never ? K : never }[MessageKey];\nexport type TranslateArgs<K extends MessageKey> = keyof MessageArguments[K] extends never ? [] : [values: MessageArguments[K]];\nexport type Translator = <K extends MessageKey>(key: K, ...args: TranslateArgs<K>) => string;\n';
  types += `export const messageParameters: Record<MessageKey, readonly string[]> = ${JSON.stringify(Object.fromEntries(modern.map(([id, g]) => [id, g.params])), null, 2)};\n`;
  let resources = '// 由 build/i18n/cli.mjs 生成，请勿手工编辑。\n';
  LANGUAGES.forEach(l => catalog.namespaces.forEach((ns, n) => { resources += `import ${l}${n} from "./locales/${l}/${ns}.json";\n`; }));
  resources += 'export const resources = {\n' + LANGUAGES.map(l => `  ${l}: { ${catalog.namespaces.map((ns, n) => `${JSON.stringify(ns)}: ${l}${n}`).join(', ')} },`).join('\n') + '\n};\n';
  resources += `export const namespaces = ${JSON.stringify(catalog.namespaces)};\n`;
  return { 'contracts.generated.ts': types, 'resources.generated.ts': resources };
}
export function generate(root) {
  const catalog = loadCatalog(root);
  const issues = structuralIssues(catalog);
  if (issues.length) throw new Error(issues.join('\n'));
  for (const [name, content] of Object.entries(generatedFiles(catalog))) fs.writeFileSync(path.join(root, name), content);
  return catalog;
}

/** 逐 key 显式确认译文；sync 不更新既有译文的确认记录。 */
export function accept(root, locale, keys, { origin, note }) {
  if (locale === SOURCE || !LANGUAGES.includes(locale)) throw new Error('Choose a supported target locale');
  if (!['human','model-authored','model-assisted','imported'].includes(origin) || !note?.trim()) throw new Error('Acceptance needs --origin and a non-empty --note');
  const catalog = loadCatalog(root), context = loadContext(root);
  const issues = structuralIssues(catalog);
  if (issues.length) throw new Error(issues.join('\n'));
  const file = path.join(root, 'metadata', `${locale}.json`);
  const records = fs.existsSync(file) ? readJson(file) : {};
  for (const id of keys) {
    const source = catalog.groups[SOURCE].get(id), target = catalog.groups[locale].get(id);
    if (!source || !target || !context[id]?.trim()) throw new Error(`Cannot accept ${id}: source, target or context is missing`);
    records[id] = { sourceHash: sourceHash(source, context[id]), targetHash: targetHash(target), state: origin === 'human' ? 'reviewed' : 'translated', origin, note };
  }
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, json(Object.fromEntries(Object.entries(records).sort(([a], [b]) => a.localeCompare(b, 'en')))));
}

/** 只添加空的目标词条，保留旧译文及确认 hash。 */
export function sync(root) {
  const sourceDir = path.join(root, 'locales', SOURCE);
  const namespaces = fs.readdirSync(sourceDir).filter(n => n.endsWith('.json')).sort();
  let added = 0;
  for (const locale of LANGUAGES.filter(l => l !== SOURCE)) {
    for (const name of namespaces) {
      const source = inspectNamespace(readJson(path.join(sourceDir, name)), SOURCE, name.slice(0, -5));
      const file = path.join(root, 'locales', locale, name);
      const target = fs.existsSync(file) ? readJson(file) : {};
      for (const group of source.values()) {
        const keys = group.plural ? new Intl.PluralRules(locale).resolvedOptions().pluralCategories.map(form => `${group.base}_${form}`) : [group.base];
        for (const key of keys) if (!own(target, key)) { target[key] = ''; added++; }
      }
      fs.mkdirSync(path.dirname(file), { recursive: true });
      fs.writeFileSync(file, json(Object.fromEntries(Object.entries(target).sort(([a],[b]) => a.localeCompare(b, 'en')))));
    }
  }
  return { added, message: 'Empty entries require translation. Obsolete entries are kept for explicit deletion. Existing acceptance hashes are unchanged.' };
}
