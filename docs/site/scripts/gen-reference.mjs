#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(siteRoot, '../..');
const generatedDir = path.join(siteRoot, 'reference/generated');
const sourceDir = path.join(siteRoot, 'reference/source');

const sourceFiles = {
  tsIndex: 'sdk/ts/src/index.ts',
  pythonSdk: 'py/python/dry/__init__.py',
  examples: 'docs/site/reference/source/examples.json',
  pages: 'docs/site/reference/source/pages.json',
  irSpec: 'docs/10-dry-ir-v0-spec.md',
  profilesReports: 'docs/11-profiles-and-reports.md',
  cliCookbook: 'docs/15-cli-cookbook.md',
  supportMatrix: 'docs/16-support-matrix.md',
};

fs.mkdirSync(generatedDir, { recursive: true });

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  return fs.readFileSync(repoPath(relativePath), 'utf8');
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

function exists(relativePath) {
  return fs.existsSync(repoPath(relativePath));
}

function hash(relativePath) {
  if (!exists(relativePath)) return null;
  return createHash('sha256').update(read(relativePath)).digest('hex');
}

function writeGenerated(name, text) {
  fs.writeFileSync(path.join(generatedDir, name), text, 'utf8');
}

function cleanComment(raw) {
  if (!raw) return '';
  return raw
    .replace(/^\/\*\*/, '')
    .replace(/\*\/$/, '')
    .split('\n')
    .map((line) => line.replace(/^\s*\* ?/, '').trimEnd())
    .join('\n')
    .trim();
}

function firstSentence(text) {
  return (text || 'No summary available.').split('\n').find(Boolean) || 'No summary available.';
}

function escapeRegex(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function escapeMarkdownInline(text) {
  return text.replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function parseTsExports() {
  const indexSource = read(sourceFiles.tsIndex);
  const exports = [];
  const groupRe = /export\s+(type\s+)?\{([\s\S]*?)\}\s+from\s+['"](.+?)['"]/g;
  let match;
  while ((match = groupRe.exec(indexSource))) {
    const isTypeOnly = Boolean(match[1]);
    const moduleRef = match[3];
    const modulePath = path.posix.join('sdk/ts/src', `${moduleRef.replace(/^\.\//, '')}.ts`);
    const names = match[2]
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean)
      .map((item) => item.replace(/\s+as\s+.+$/, '').trim());

    for (const name of names) {
      exports.push({ name, modulePath, isTypeOnly });
    }
  }
  return exports.sort((a, b) => a.name.localeCompare(b.name));
}

function findTsDeclaration(exp) {
  if (!exists(exp.modulePath)) {
    return { ...exp, kind: exp.isTypeOnly ? 'type' : 'value', summary: 'Source file not found.', signature: '' };
  }

  const source = read(exp.modulePath);
  const declRe = new RegExp(
    `((?:/\\*\\*[\\s\\S]*?\\*/\\s*)?)export\\s+(class|interface|type|const|function)\\s+${escapeRegex(exp.name)}\\b[^\\n{;=]*`,
    'm',
  );
  const match = source.match(declRe);
  if (!match) {
    return {
      ...exp,
      kind: exp.isTypeOnly ? 'type' : 'value',
      summary: 'No exported declaration found in source file.',
      signature: '',
    };
  }

  const doc = cleanComment(match[1]);
  const signature = source.slice(match.index + match[1].length).split('\n')[0].trim();
  return {
    ...exp,
    kind: match[2],
    summary: firstSentence(doc),
    doc,
    signature,
  };
}

function findPrecedingBlockDoc(source, index) {
  const before = source.slice(0, index);
  const end = before.lastIndexOf('*/');
  const start = before.lastIndexOf('/**');
  if (start === -1 || end === -1 || end < start) return '';
  if (before.slice(end + 2).trim()) return '';
  return cleanComment(before.slice(start, end + 2));
}

function extractTsDesignMethods() {
  const source = read('sdk/ts/src/design.ts');
  const classIndex = source.indexOf('export class Design');
  if (classIndex === -1) return [];
  const openIndex = source.indexOf('{', classIndex);
  let depth = 0;
  let closeIndex = openIndex;
  for (let i = openIndex; i < source.length; i += 1) {
    if (source[i] === '{') depth += 1;
    if (source[i] === '}') depth -= 1;
    if (depth === 0) {
      closeIndex = i;
      break;
    }
  }

  const body = source.slice(openIndex + 1, closeIndex);
  const methods = [];
  const methodRe = /^  ([a-zA-Z][a-zA-Z0-9_]*)\(([\s\S]*?)\):\s*([^{]+)\{/gm;
  let match;
  while ((match = methodRe.exec(body))) {
    const name = match[1];
    if (name.startsWith('_')) continue;
    const absoluteIndex = openIndex + 1 + match.index;
    const params = match[2].replace(/\s+/g, ' ').trim();
    const returns = match[3].replace(/\s+/g, ' ').trim();
    const doc = findPrecedingBlockDoc(source, absoluteIndex);
    methods.push({
      name,
      signature: `${name}(${params}): ${returns}`,
      summary: firstSentence(doc),
    });
  }
  return methods;
}

function parsePythonApi() {
  const code = String.raw`
import ast
import json
import sys

source = open(sys.argv[1], encoding="utf-8").read()
tree = ast.parse(source)

def unparse(node):
    if node is None:
        return ""
    return ast.unparse(node)

def public_all():
    for node in tree.body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name) and target.id == "__all__":
                    return [elt.value for elt in node.value.elts if isinstance(elt, ast.Constant)]
    return []

def signature(fn):
    args = []
    positional = list(fn.args.posonlyargs) + list(fn.args.args)
    defaults = [None] * (len(positional) - len(fn.args.defaults)) + list(fn.args.defaults)
    for arg, default in zip(positional, defaults):
        text = arg.arg
        if arg.annotation is not None:
            text += ": " + unparse(arg.annotation)
        if default is not None:
            text += " = " + unparse(default)
        args.append(text)
    if fn.args.vararg:
        text = "*" + fn.args.vararg.arg
        if fn.args.vararg.annotation is not None:
            text += ": " + unparse(fn.args.vararg.annotation)
        args.append(text)
    if fn.args.kwonlyargs:
        if not fn.args.vararg:
            args.append("*")
        for arg, default in zip(fn.args.kwonlyargs, fn.args.kw_defaults):
            text = arg.arg
            if arg.annotation is not None:
                text += ": " + unparse(arg.annotation)
            if default is not None:
                text += " = " + unparse(default)
            args.append(text)
    if fn.args.kwarg:
        text = "**" + fn.args.kwarg.arg
        if fn.args.kwarg.annotation is not None:
            text += ": " + unparse(fn.args.kwarg.annotation)
        args.append(text)
    result = "def " + fn.name + "(" + ", ".join(args) + ")"
    if fn.returns is not None:
        result += " -> " + unparse(fn.returns)
    return result

assignments = {}
classes = []
functions = []

for node in tree.body:
    if isinstance(node, ast.Assign):
        for target in node.targets:
            if isinstance(target, ast.Name):
                assignments[target.id] = unparse(node.value)
    elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
        assignments[node.target.id] = unparse(node.annotation)
    elif isinstance(node, ast.ClassDef):
        classes.append({
            "name": node.name,
            "doc": ast.get_docstring(node) or "",
            "methods": [
                {
                    "name": item.name,
                    "signature": signature(item),
                    "doc": ast.get_docstring(item) or "",
                }
                for item in node.body
                if isinstance(item, ast.FunctionDef) and not item.name.startswith("_")
            ],
        })
    elif isinstance(node, ast.FunctionDef):
        functions.append({
            "name": node.name,
            "signature": signature(node),
            "doc": ast.get_docstring(node) or "",
        })

print(json.dumps({
    "all": public_all(),
    "assignments": assignments,
    "classes": classes,
    "functions": functions,
}))
`;

  const pythonPath = repoPath(sourceFiles.pythonSdk);
  const bins = ['python3', 'python'];
  for (const bin of bins) {
    try {
      return JSON.parse(execFileSync(bin, ['-', pythonPath], { input: code, encoding: 'utf8' }));
    } catch {
      // Try the next Python binary.
    }
  }
  throw new Error('Unable to run python3 or python for Python API extraction');
}

function extractHeadings(relativePath) {
  if (!exists(relativePath)) return [];
  const headings = [];
  let inFence = false;
  for (const line of read(relativePath).split('\n')) {
    if (/^```/.test(line.trim())) {
      inFence = !inFence;
      continue;
    }
    if (inFence || !/^#{1,3} /.test(line)) continue;
    headings.push(line);
  }
  return headings.map((line) => {
      const level = line.match(/^#+/)[0].length;
      const text = line.replace(/^#+\s+/, '').trim();
      return { level, text };
    });
}

function renderHeadingIndex(title, sourcePath, intro) {
  const headings = extractHeadings(sourcePath);
  const lines = [banner(), `# ${title}`, '', intro, '', `Source: \`${sourcePath}\``, ''];
  if (headings.length) {
    lines.push('## Source sections', '');
    for (const heading of headings) {
      const indent = '  '.repeat(Math.max(0, heading.level - 1));
      lines.push(`${indent}- ${escapeMarkdownInline(heading.text)}`);
    }
    lines.push('');
  }
  return lines.join('\n');
}

function banner() {
  return '<!-- Generated by docs/site/scripts/gen-reference.mjs. Do not edit by hand. -->\n';
}

function renderTsReference(exports) {
  const declarations = exports.map(findTsDeclaration);
  const designMethods = extractTsDesignMethods();
  const lines = [
    banner(),
    '# TypeScript SDK',
    '',
    `Generated from \`${sourceFiles.tsIndex}\` and public re-export sources.`,
    '',
    '## Public exports',
    '',
    '| Export | Kind | Source | Summary |',
    '| --- | --- | --- | --- |',
  ];

  for (const item of declarations) {
    lines.push(`| \`${item.name}\` | ${item.kind} | \`${item.modulePath}\` | ${item.summary.replace(/\|/g, '\\|')} |`);
  }

  for (const item of declarations) {
    lines.push('', `## \`${item.name}\``, '', `Source: \`${item.modulePath}\``, '');
    if (item.signature) {
      lines.push('```ts', item.signature, '```', '');
    }
    lines.push(item.doc || item.summary, '');
    if (item.name === 'Design' && designMethods.length) {
      lines.push('### Methods', '', '| Method | Signature | Summary |', '| --- | --- | --- |');
      for (const method of designMethods) {
        lines.push(`| \`${method.name}\` | \`${method.signature.replace(/\|/g, '\\|')}\` | ${method.summary.replace(/\|/g, '\\|')} |`);
      }
      lines.push('');
    }
  }

  return lines.join('\n');
}

function renderPythonReference(api) {
  const classMap = new Map(api.classes.map((item) => [item.name, item]));
  const functionMap = new Map(api.functions.map((item) => [item.name, item]));
  const lines = [
    banner(),
    '# Python SDK',
    '',
    `Generated from \`${sourceFiles.pythonSdk}\` using Python AST extraction.`,
    '',
    '## Public names',
    '',
    '| Name | Kind | Summary |',
    '| --- | --- | --- |',
  ];

  for (const name of api.all) {
    let kind = 'value';
    let summary = '';
    if (classMap.has(name)) {
      kind = 'class';
      summary = firstSentence(classMap.get(name).doc);
    } else if (functionMap.has(name)) {
      kind = 'function';
      summary = firstSentence(functionMap.get(name).doc);
    } else if (api.assignments[name]) {
      kind = new Set(['PRINTERS', 'TPMS_SURFACES']).has(name) ? 'constant' : 'type alias';
      summary = `\`${api.assignments[name]}\``;
    }
    lines.push(`| \`${name}\` | ${kind} | ${String(summary || 'No summary available.').replace(/\|/g, '\\|')} |`);
  }

  for (const klass of api.classes.filter((item) => api.all.includes(item.name))) {
    lines.push('', `## \`${klass.name}\``, '', klass.doc || 'No summary available.', '', '### Methods', '');
    lines.push('| Method | Signature | Summary |', '| --- | --- | --- |');
    for (const method of klass.methods) {
      lines.push(`| \`${method.name}\` | \`${method.signature.replace(/\|/g, '\\|')}\` | ${firstSentence(method.doc).replace(/\|/g, '\\|')} |`);
    }
    lines.push('');
  }

  for (const fn of api.functions.filter((item) => api.all.includes(item.name))) {
    lines.push('', `## \`${fn.name}\``, '', '```py', fn.signature, '```', '', fn.doc || 'No summary available.', '');
  }

  return lines.join('\n');
}

function renderExamples(examples) {
  const lines = [
    banner(),
    '# Examples',
    '',
    `Generated from \`${sourceFiles.examples}\`.`,
    '',
    '| Example | Guide | Languages | Sources | Outputs | Concepts |',
    '| --- | --- | --- | --- | --- | --- |',
  ];

  for (const example of examples) {
    const sources = Object.entries(example.sources)
      .map(([language, source]) => `${language}: \`${source}\``)
      .join('<br>');
    lines.push(
      `| ${example.title} | [/${example.slug}](/guide/${example.slug}) | ${example.languages.join(', ')} | ${sources} | ${example.outputs.join(', ')} | ${example.concepts.map((item) => `\`${item}\``).join(', ')} |`,
    );
  }

  return lines.join('\n');
}

function renderGenerators(exports) {
  const generatorExports = exports
    .filter((item) => item.modulePath.includes('/generators/'))
    .map(findTsDeclaration);

  const lines = [
    banner(),
    '# Generators',
    '',
    'Generated from TypeScript generator exports.',
    '',
    '| Export | Source | Summary |',
    '| --- | --- | --- |',
  ];

  for (const item of generatorExports) {
    lines.push(`| \`${item.name}\` | \`${item.modulePath}\` | ${item.summary.replace(/\|/g, '\\|')} |`);
  }
  lines.push('');
  return lines.join('\n');
}

function renderVerification(exports) {
  const names = new Set(['Report', 'Finding', 'Severity', 'ToolpathMeta']);
  const items = exports.filter((item) => names.has(item.name)).map(findTsDeclaration);
  const lines = [
    banner(),
    '# Verification',
    '',
    'Generated from public report-related SDK types.',
    '',
    '| Export | Source | Summary |',
    '| --- | --- | --- |',
  ];
  for (const item of items) {
    lines.push(`| \`${item.name}\` | \`${item.modulePath}\` | ${item.summary.replace(/\|/g, '\\|')} |`);
  }
  lines.push('');
  lines.push('## Related source documentation', '');
  lines.push('- `docs/11-profiles-and-reports.md`');
  lines.push('- `docs/16-support-matrix.md`');
  lines.push('');
  return lines.join('\n');
}

function writeManifest() {
  const manifest = {
    generator: 'docs/site/scripts/gen-reference.mjs',
    sources: Object.fromEntries(Object.entries(sourceFiles).map(([key, relativePath]) => [key, {
      path: relativePath,
      sha256: hash(relativePath),
    }])),
  };
  writeGenerated('manifest.json', `${JSON.stringify(manifest, null, 2)}\n`);
}

const tsExports = parseTsExports();
const pythonApi = parsePythonApi();
const examples = readJson(sourceFiles.examples);

writeGenerated('typescript-sdk.md', renderTsReference(tsExports));
writeGenerated('python-sdk.md', renderPythonReference(pythonApi));
writeGenerated('cli.md', renderHeadingIndex('CLI', sourceFiles.cliCookbook, 'Generated index of command-line documentation source sections.'));
writeGenerated('ir.md', renderHeadingIndex('IR', sourceFiles.irSpec, 'Generated index of the Dry IR specification source sections.'));
writeGenerated('profiles-and-reports.md', renderHeadingIndex('Profiles and reports', sourceFiles.profilesReports, 'Generated index of profile and report documentation source sections.'));
writeGenerated('generators.md', renderGenerators(tsExports));
writeGenerated('verification.md', renderVerification(tsExports));
writeGenerated('examples.md', renderExamples(examples));
writeManifest();
