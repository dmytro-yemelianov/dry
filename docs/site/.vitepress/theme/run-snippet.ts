import { transform } from 'sucrase';
import type { Dry } from './dry-engine';

const KEYS = [
  'Design',
  'PRINTERS',
  'resolveGcode',
  'resolveMetrics',
  'resolveMetricsIr',
  'resolveIr',
  'resolveBinary',
  'resolveOptimizedIr',
  'resolveBalancedIr',
  'resolveVerify',
  'tpms',
  'starPolygonLattice',
] as const;

export function compileSnippet(src: string): (dry: Dry) => unknown {
  const aliases = importAliases(src);
  const stripped = stripModuleSyntax(src);
  const js = transform(stripped, { transforms: ['typescript'] }).code;
  const destructure = `const { ${KEYS.join(', ')} } = __dry;\n`;
  const aliasLines = aliases
    .map(({ imported, local }) => imported ? `const ${local} = __dry.${imported};` : `const ${local} = __dry;`)
    .join('\n');
  const body = `${destructure}${aliasLines ? `${aliasLines}\n` : ''}return (function(){\n${wrapReturn(js)}\n})();`;
  return new Function('__dry', `'use strict';\n${body}`) as (dry: Dry) => unknown;
}

function importAliases(src: string): Array<{ imported: string; local: string }> {
  const aliases: Array<{ imported: string; local: string }> = [];
  const named = /^\s*import\s+\{([^}]+)\}\s+from\s+['"]@dry\/sdk['"];?\s*$/gm;
  for (const match of src.matchAll(named)) {
    for (const item of match[1].split(',')) {
      const [imported, local = imported] = item.trim().split(/\s+as\s+/);
      if (imported && local && imported !== local) aliases.push({ imported: imported.trim(), local: local.trim() });
    }
  }
  const namespace = /^\s*import\s+\*\s+as\s+([A-Za-z_$][\w$]*)\s+from\s+['"]@dry\/sdk['"];?\s*$/gm;
  for (const match of src.matchAll(namespace)) aliases.push({ imported: '', local: match[1] });
  return aliases;
}

function stripModuleSyntax(src: string): string {
  return src
    .replace(/^\s*import\s+[^;\n]+from\s+['"]@dry\/sdk['"];?\s*$/gm, '')
    .replace(/^\s*export\s+\{[^}]+\};?\s*$/gm, '')
    .replace(/\bexport\s+default\s+/g, '')
    .replace(/\bexport\s+(?=(const|let|var|function|class)\b)/g, '');
}

function wrapReturn(js: string): string {
  const cleaned = js
    .split('\n')
    .filter((line) => !/^\s*"use strict";\s*$/.test(line))
    .join('\n')
    .trim();
  if (!cleaned) return 'return undefined;';
  if (/^\s*(return|throw|if|for|while|switch|try|class|function)\b/.test(cleaned)) return cleaned;

  const body = trimTrailingSemicolons(cleaned);
  const split = lastTopLevelSemicolon(body);
  if (split === -1) {
    if (/^\s*(const|let|var)\b/.test(body)) return `${body};\nreturn undefined;`;
    return `return (${body});`;
  }

  const head = body.slice(0, split + 1);
  const tail = body.slice(split + 1).trim();
  if (!tail || /^\s*(const|let|var|return|throw)\b/.test(tail)) return `${body};\nreturn undefined;`;
  return `${head}\nreturn (${tail});`;
}

function trimTrailingSemicolons(code: string): string {
  let end = code.length;
  while (end > 0 && /[\s;]/.test(code[end - 1])) end--;
  return code.slice(0, end);
}

function lastTopLevelSemicolon(code: string): number {
  let depth = 0;
  let quote: string | null = null;
  let escaped = false;
  let lineComment = false;
  let blockComment = false;
  let last = -1;

  for (let i = 0; i < code.length; i++) {
    const ch = code[i];
    const next = code[i + 1];

    if (lineComment) {
      if (ch === '\n') lineComment = false;
      continue;
    }
    if (blockComment) {
      if (ch === '*' && next === '/') {
        blockComment = false;
        i++;
      }
      continue;
    }
    if (quote) {
      if (escaped) escaped = false;
      else if (ch === '\\') escaped = true;
      else if (ch === quote) quote = null;
      continue;
    }
    if (ch === '/' && next === '/') {
      lineComment = true;
      i++;
      continue;
    }
    if (ch === '/' && next === '*') {
      blockComment = true;
      i++;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      quote = ch;
      continue;
    }
    if (ch === '(' || ch === '[' || ch === '{') depth++;
    else if (ch === ')' || ch === ']' || ch === '}') depth = Math.max(0, depth - 1);
    else if (ch === ';' && depth === 0) last = i;
  }
  return last;
}

export function runSnippet(src: string, dry: Dry):
  | { ok: true; value: unknown }
  | { ok: false; error: string } {
  try {
    return { ok: true, value: compileSnippet(src)(dry) };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}
