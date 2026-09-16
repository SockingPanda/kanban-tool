import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { ts } from './typescript.mjs';

const copyAttributes = new Set(['aria-label','aria-description','aria-valuetext','title','placeholder','alt','label','description','emptyMessage','helperText','message','tooltip']);
const human = text => /\p{L}/u.test(text);
const cjk = text => /[\u3400-\u9fff]/u.test(text);
const normalize = text => text.replace(/\s+/g, ' ').trim();

/** Conservative AST inventory, not a proof that every visible string was discovered. */
export function scanText(text, filename = 'component.tsx') {
  const tree = ts.createSourceFile(filename, text, ts.ScriptTarget.Latest, true, filename.endsWith('tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  if (tree.parseDiagnostics.length) throw new Error(`${filename}: source syntax is invalid`);
  const findings = [], seen = new Set();
  function add(node, kind, value) {
    const normalized = normalize(value);
    if (!human(normalized)) return;
    const position = node.getStart(tree);
    if (seen.has(position)) return;
    seen.add(position);
    const { line, character } = tree.getLineAndCharacterOfPosition(position);
    findings.push({ file: filename.replaceAll('\\','/'), line: line + 1, column: character + 1, kind, text: normalized });
  }
  function translated(node) {
    // Do not treat explicit translation keys as display copy.
    const parent = node.parent;
    return ts.isCallExpression(parent) && (parent.expression.getText(tree) === 't' || parent.expression.getText(tree).endsWith('.t')) && parent.arguments[0] === node;
  }
  function displayExpression(node) {
    if (ts.isStringLiteralLike(node)) { add(node, 'jsx-expression', node.text); return; }
    if (ts.isConditionalExpression(node)) { displayExpression(node.whenTrue); displayExpression(node.whenFalse); return; }
    if (ts.isBinaryExpression(node)) {
      const op = node.operatorToken.kind;
      if ([ts.SyntaxKind.PlusToken, ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(op)) { displayExpression(node.left); displayExpression(node.right); }
      if (op === ts.SyntaxKind.AmpersandAmpersandToken) displayExpression(node.right);
      return;
    }
    if (ts.isTemplateExpression(node)) {
      add(node.head, 'jsx-template', node.head.text);
      for (const span of node.templateSpans) add(span.literal, 'jsx-template', span.literal.text);
    }
    // Calls, identifiers and property access are not statically known UI copy.
  }
  function visit(node) {
    if (ts.isJsxText(node)) add(node, 'jsx-text', node.text);
    if (ts.isJsxAttribute(node) && copyAttributes.has(node.name.getText(tree))) {
      if (node.initializer && ts.isStringLiteral(node.initializer)) add(node.initializer, 'jsx-attribute', node.initializer.text);
      if (node.initializer && ts.isJsxExpression(node.initializer) && node.initializer.expression) displayExpression(node.initializer.expression);
    }
    if (ts.isJsxExpression(node) && !ts.isJsxAttribute(node.parent) && node.expression) displayExpression(node.expression);
    if (ts.isPropertyAssignment(node) && copyAttributes.has(node.name.getText(tree).replace(/^['"]|['"]$/g,'')) && ts.isStringLiteralLike(node.initializer) && !/^[a-z][a-z0-9-]*:[A-Za-z]/.test(node.initializer.text)) add(node.initializer, 'copy-property', node.initializer.text);
    if (ts.isStringLiteralLike(node) && cjk(node.text) && !translated(node)) add(node, 'cjk-literal', node.text);
    if (ts.isTemplateHead(node) || ts.isTemplateMiddle(node) || ts.isTemplateTail(node)) if (cjk(node.text)) add(node, 'cjk-template', node.text);
    ts.forEachChild(node, visit);
  }
  visit(tree);
  return findings.sort((a,b) => a.line-b.line || a.column-b.column);
}
export function fingerprint(finding) { return `${finding.file}|${finding.text}`; }
export function allowed(findings, allowlist) {
  for (const item of allowlist) if (!item.file || !item.text || !item.reason?.trim()) throw new Error('Every allowlist entry needs a file, exact text, and reason');
  return findings.filter(f => !allowlist.some(a => a.file === f.file && a.text === f.text));
}
export function introduced(before, after) {
  const counts = new Map();
  for (const f of before) counts.set(fingerprint(f), (counts.get(fingerprint(f)) ?? 0) + 1);
  return after.filter(f => {
    const key = fingerprint(f), remaining = counts.get(key) ?? 0;
    if (!remaining) return true;
    counts.set(key, remaining - 1);
    return false;
  });
}
export function listSources(src) {
  const files = [];
  function visit(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isSymbolicLink()) continue;
      if (entry.isDirectory()) {
        if (['node_modules','generated','localization'].includes(entry.name)) continue;
        visit(full);
      } else if (/\.(ts|tsx)$/.test(entry.name) && !/\.(test|spec|generated|d)\.(ts|tsx)$/.test(entry.name)) files.push(full);
    }
  }
  visit(src); return files.sort();
}
export function scanRepository(webRoot, { baseRef, strictFiles = [], allowlist = [] } = {}) {
  const files = listSources(path.join(webRoot,'src'));
  const findings = [];
  let previous = [];
  let gitRoot;
  if (baseRef) {
    gitRoot = execFileSync('git',['rev-parse','--show-toplevel'],{cwd:webRoot,encoding:'utf8'}).trim();
    if (!/^[0-9a-f]{40}$/.test(baseRef)) throw new Error('Use a full 40-character base commit SHA');
    execFileSync('git',['cat-file','-e',`${baseRef}^{commit}`],{cwd:gitRoot,stdio:'pipe'});
  }
  for (const file of files) {
    const relative = path.relative(webRoot,file).replaceAll('\\','/');
    findings.push(...scanText(fs.readFileSync(file,'utf8'),relative));
    if (baseRef) {
      const repoPath = path.relative(gitRoot,file).replaceAll('\\','/');
      const present = execFileSync('git',['ls-tree',baseRef,'--',repoPath],{cwd:gitRoot,encoding:'utf8'}).trim();
      if (present) {
        const original = execFileSync('git',['show',`${baseRef}:${repoPath}`],{cwd:gitRoot,encoding:'utf8',maxBuffer:16*1024*1024});
        previous.push(...scanText(original,relative));
      }
    }
  }
  const remaining = allowed(findings,allowlist);
  const strictErrors = remaining.filter(f => strictFiles.includes(f.file));
  const newErrors = baseRef ? introduced(allowed(previous,allowlist),remaining) : [];
  return { scope: webRoot, files: files.length, findings: remaining, strictErrors, missingStrictFiles: strictFiles.filter(file => !fs.existsSync(path.join(webRoot,file))), introduced: newErrors, baselineCompared: baseRef ?? null };
}
