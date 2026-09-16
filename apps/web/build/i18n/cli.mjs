import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { checkCatalog, generate, accept, sync, json, readJson } from './catalog.mjs';
import { scanRepository } from './scan.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const command = argv[0] ?? 'check';
function option(name, fallback) { const i=argv.indexOf(name); if(i<0)return fallback; if(!argv[i+1]||argv[i+1].startsWith('--'))throw new Error(`${name} needs a value`);return argv[i+1]; }
const webRoot = path.resolve(option('--web-root',path.join(here,'../..')));
const root = path.resolve(option('--root',path.join(webRoot,'src/platform/localization')));
try {
  let result;
  if (command === 'generate') {
    const catalog = generate(root);
    result = { command, logicalKeys: catalog.groups.zh.size, namespaces: catalog.namespaces };
  } else if (command === 'check') {
    const { errors, logicalKeys, physicalKeys } = checkCatalog(root);
    const config = readJson(path.join(here,'config.json'));
    const scan = scanRepository(webRoot,config);
    result = { command, logicalKeys, physicalKeys, errors, sourceFilesScanned:scan.files, strictErrors:scan.strictErrors, missingStrictFiles:scan.missingStrictFiles, debtFindings:scan.findings.length };
    if(errors.length || scan.strictErrors.length || scan.missingStrictFiles.length)process.exitCode=1;
  } else if (command === 'report' || command === 'scan') {
    const config=readJson(path.join(here,'config.json'));
    result=scanRepository(webRoot,{...config,baseRef:option('--base')});
    if(command==='scan' && (result.strictErrors.length || result.missingStrictFiles.length || result.introduced.length))process.exitCode=1;
  } else if (command === 'sync') {
    result=sync(root);
  } else if (command === 'accept') {
    const locale=option('--locale','en');
    const key=option('--key');
    // Deliberately no --all in the public CLI. Acceptance must be scoped to a translation.
    if(!key)throw new Error('Use --key namespace:key, --origin human|model-assisted|model-authored|imported, and --note');
    accept(root,locale,[key],{origin:option('--origin'),note:option('--note')});
    result={command,locale,key};
  } else throw new Error(`Unknown command ${command}`);
  const output=option('--output');
  if(output)fs.writeFileSync(path.resolve(output),json(result));
  console.log(json(result));
} catch(error) { console.error(error.stack ?? error.message);process.exitCode=1; }
