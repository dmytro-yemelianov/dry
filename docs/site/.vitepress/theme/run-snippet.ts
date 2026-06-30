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
  const named = /\bimport\s+\{([^}]+)\}\s+from\s*=?\s*['"]@dry\/sdk['"];?/g;
  for (const match of src.matchAll(named)) {
    for (const item of match[1].split(',')) {
      const [imported, local = imported] = item.trim().split(/\s+as\s+/);
      if (imported && local && imported !== local) aliases.push({ imported: imported.trim(), local: local.trim() });
    }
  }
  const namespace = /\bimport\s+\*\s+as\s+([A-Za-z_$][\w$]*)\s+from\s*=?\s*['"]@dry\/sdk['"];?/g;
  for (const match of src.matchAll(namespace)) aliases.push({ imported: '', local: match[1] });
  return aliases;
}

function stripModuleSyntax(src: string): string {
  return src
    .replace(/\bimport\s+\{[^}]+\}\s+from\s*=?\s*['"]@dry\/sdk['"];?/g, '')
    .replace(/\bimport\s+\*\s+as\s+[A-Za-z_$][\w$]*\s+from\s*=?\s*['"]@dry\/sdk['"];?/g, '')
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
  if (/^\s*(return|throw)\b/.test(cleaned)) return cleaned;

  const body = trimTrailingSemicolons(cleaned);
  const split = lastTopLevelStatementStart(body);
  if (split === 0) return shouldReturn(body) ? `return (${body});` : `${body};\nreturn undefined;`;

  const head = body.slice(0, split).trimEnd();
  const tail = body.slice(split).trim();
  if (!tail || !shouldReturn(tail)) return `${body};\nreturn undefined;`;
  return `${head}\nreturn (${tail});`;
}

function shouldReturn(statement: string): boolean {
  return !/^\s*(const|let|var|return|throw|if|else|for|while|switch|try|catch|finally|class|function)\b/.test(statement);
}

function trimTrailingSemicolons(code: string): string {
  let end = code.length;
  while (end > 0 && /[\s;]/.test(code[end - 1])) end--;
  return code.slice(0, end);
}

function lastTopLevelStatementStart(code: string): number {
  let depth = 0;
  let quote: string | null = null;
  let escaped = false;
  let lineComment = false;
  let lineCommentPrev = '';
  let blockComment = false;
  let last = 0;

  for (let i = 0; i < code.length; i++) {
    const ch = code[i];
    const next = code[i + 1];

    if (lineComment) {
      if (ch === '\n') {
        lineComment = false;
        if (depth === 0) {
          const start = nextNonWhitespace(code, i + 1);
          if (start !== -1 && canEndStatement(lineCommentPrev, code[start]) && !continuesClause(code, start)) {
            last = start;
          }
        }
      }
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
      lineCommentPrev = previousNonWhitespace(code, i - 1);
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
    else if ((ch === ';' || ch === '\n') && depth === 0) {
      const start = nextNonWhitespace(code, i + 1);
      if (start !== -1 && !continuesClause(code, start) && (ch === ';' || canEndStatement(previousNonWhitespace(code, i - 1), code[start]))) {
        last = start;
      }
    }
  }
  return last;
}

function previousNonWhitespace(code: string, start: number): string {
  for (let i = start; i >= 0; i--) {
    if (!/\s/.test(code[i])) return code[i];
  }
  return '';
}

function nextNonWhitespace(code: string, start: number): number {
  for (let i = start; i < code.length; i++) {
    if (!/\s/.test(code[i])) return i;
  }
  return -1;
}

function canEndStatement(prev: string, next: string): boolean {
  return !!prev && /[\w$)\]}'"`0-9]/.test(prev) && !/[.[(,?:+\-*/%&|^]/.test(next);
}

function continuesClause(code: string, start: number): boolean {
  return /^(else|catch|finally)\b/.test(code.slice(start));
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
