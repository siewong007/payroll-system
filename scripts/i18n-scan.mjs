#!/usr/bin/env node
/**
 * i18n hardcoded-string scanner — audit tool + CI gate.
 *
 * Frontend pass scans all .ts/.tsx under src/ for user-facing literals that
 * bypass the i18n system:
 *   - JSX text nodes containing letters        (>Save</button>)
 *   - user-facing JSX attributes with literals (placeholder= title=
 *     aria-label= alt= label= heading= description= emptyText=)
 *   - display-bearing object properties        (name: '…', label: '…',
 *     title: '…', description: '…', message: '…', text: '…')
 *   - message calls                            (setError('…'), alert('…'),
 *     confirm('…'), getErrorMessage(err, '…'), throw new Error('…'))
 *   - locale-pinned formatters                 (toLocaleString/DateString/
 *     TimeString, new Intl.* with a literal locale)
 *
 * Backend pass extracts AppError::Variant("literal") user-facing strings from
 * backend/src and checks each against frontend/src/i18n/serverErrorMap.ts.
 *
 * Usage: node scripts/i18n-scan.mjs [--ci] [--json]
 *   --ci    exit 1 when frontend findings exist (backend unmapped = warning)
 *   --json  machine-readable report on stdout
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(fileURLToPath(new URL('.', import.meta.url)), '..');
const FE_SRC = join(ROOT, 'frontend', 'src');
const BE_SRC = join(ROOT, 'backend', 'src');

const SKIP_DIRS = new Set(['node_modules', 'dist', 'tests', 'locales', '.git']);
const SKIP_FILES = new Set(['serverErrorMap.ts']);

// Attributes whose values are always user-visible when they hold literals.
const USER_ATTRS = [
  'placeholder', 'title', 'aria-label', 'alt', 'label', 'heading',
  'description', 'emptyText', 'tooltip', 'caption', 'subheading',
];
// Object-literal properties that normally carry display copy.
const DISPLAY_PROPS = ['name', 'label', 'title', 'heading', 'description', 'message', 'text', 'emptyMessage', 'confirmText'];
// Calls whose first/last string argument is a user-facing message.
const MESSAGE_CALLS = [
  'setError', 'setMessage', 'setSuccess', 'setSuccessMessage', 'setErrorMessage',
  'setNotice', 'setBanner', 'setSubmitError', 'setFormError',
  'alert', 'confirm', 'getErrorMessage', 'toast', 'notify',
];
// Lines where literals are never user-facing copy.
const SAFE_LINE = /\b(import|export|from|require|console\.|className=|data-|key=|to=|href=|path=|type=|id=|name=|role=|autoComplete|autoFocus|method=|action=|src=|variant=|size=|target=|rel=|viewBox|xmlns|fill=|stroke=|d=|cx=|cy=|r=|x=|y=|width=|height=|layoutId=|initial=|animate=|exit=|transition=|whileHover|whileTap|ease=|duration=|queryKey|queryFn|staleTime|mutationKey|column|accessorKey|header: t\(|cell:|enableSorting|resolve|reject|throw new (TypeError|RangeError)|expect\(|vi\.|it\(|describe\(|test\(|localStorage|sessionStorage|navigator\.|document\.|window\.|process\.env|import\.meta)\b/;
const CSSY = /^[a-z0-9-:/[\]()._%\s]+$/; // tailwind-ish class strings
const URLISH = /^(https?:|\/|#|mailto:|tel:|[a-z-]+\/)/i;
const HAS_LETTER = /[A-Za-z\u00C0-\u024F\u4E00-\u9FFF]/;
// Snake_case / camelCase identifiers and enums are not copy.
const IDENTIFIER = /^[a-z][a-zA-Z0-9]*(_[a-z0-9]+)*$|^[A-Z][A-Z0-9_]+$/;
// Punctuation-only / numeric / single-char strings.
const TRIVIAL = /^[\s\W_]*$|^[\d\s.,:%/+-]+$/;

function* walk(dir) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (SKIP_DIRS.has(entry)) continue;
    const st = statSync(full);
    if (st.isDirectory()) yield* walk(full);
    else yield full;
  }
}

function cleanText(s) {
  return s
    .replace(/\\u([0-9a-fA-F]{4})/g, (_, h) => String.fromCharCode(parseInt(h, 16)))
    .replace(/\\[nt]/g, ' ')
    .replace(/&\w+;/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function isCopyText(s) {
  const t = cleanText(s);
  if (t.length < 2) return false;
  if (!HAS_LETTER.test(t)) return false;
  if (TRIVIAL.test(t)) return false;
  if (URLISH.test(t)) return false;
  if (IDENTIFIER.test(t) && !/\s/.test(t)) return false;
  if (CSSY.test(t) && t.includes('-') && !/\s{2,}/.test(t) && t.split(' ').every(w => !/^[A-Z]/.test(w))) return false;
  return true;
}

function scanFrontendFile(file) {
  const findings = [];
  const lines = readFileSync(file, 'utf8').split('\n');
  let inBlockComment = false;
  for (let i = 0; i < lines.length; i++) {
    let line = lines[i];
    // `// i18n-ok` on a line opts it out — for intentional raw text like
    // internal 'en-GB' parsing or brand names that never translate.
    if (line.includes('i18n-ok') || (lines[i - 1] ?? '').includes('i18n-ok')) continue;
    // strip block comments (crude but adequate for this codebase)
    let out = '';
    for (let j = 0; j < line.length; j++) {
      if (inBlockComment) {
        if (line[j] === '*' && line[j + 1] === '/') { inBlockComment = false; j++; }
        continue;
      }
      if (line[j] === '/' && line[j + 1] === '*') { inBlockComment = true; j++; continue; }
      if (line[j] === '/' && line[j + 1] === '/') break;
      out += line[j];
    }
    line = out;
    if (!line.trim()) continue;
    const isSafe = SAFE_LINE.test(line);

    // 1. JSX text nodes: >text< or >text{ or }text<
    for (const m of line.matchAll(/>([^<>{}]*[A-Za-z\u4E00-\u9FFF][^<>{}]*)</g)) {
      const txt = cleanText(m[1]);
      // Type positions: `=> Promise<X>`, `): Partial<X>`, `: Column<X>` all
      // match the pattern above but are generics, not JSX text.
      const before = line.slice(0, m.index);
      if (/[=):]\s*$/.test(before) || /\btype\s*$/.test(before)) continue;
      // `Record<X>).id) return …` — generic close followed by more code.
      if (/^[)(]/.test(txt)) continue;
      // Skip ternary fragments inside JSX expressions: `> 0 ? 'cls' : x <`
      if (/\?\s*['"]|['"]\s*:/.test(txt)) continue;
      // Skip code comparisons caught between `>` and `<`: `i >= 1 && i <= 5`.
      // `&&`/`||` are JS operators and cannot appear in literal JSX text.
      if (/&&|\|\|/.test(txt)) continue;
      if (isCopyText(txt)) {
        findings.push({ file, line: i + 1, kind: 'jsx-text', text: txt });
      }
    }
    // 2. User-facing attributes with literal values
    for (const attr of USER_ATTRS) {
      const re = new RegExp(`(?<![\\w-])${attr}=\\{?\\s*(['"\`])((?:\\\\.|(?!\\1).)+)\\1`, 'g');
      for (const m of line.matchAll(re)) {
        const txt = cleanText(m[2]);
        if (isCopyText(txt) && !m[0].includes('t(')) {
          findings.push({ file, line: i + 1, kind: `attr:${attr}`, text: txt });
        }
      }
    }
    if (isSafe) continue;
    // 3. Display-bearing object properties: label: 'X'
    for (const prop of DISPLAY_PROPS) {
      const re = new RegExp(`\\b${prop}:\\s*(['"])((?:\\\\.|(?!\\1).)+)\\1`, 'g');
      for (const m of line.matchAll(re)) {
        const txt = cleanText(m[2]);
        if (isCopyText(txt) && !/^[a-z_]+$/.test(txt)) {
          findings.push({ file, line: i + 1, kind: `prop:${prop}`, text: txt });
        }
      }
    }
    // 4. Message calls: setError('X'), getErrorMessage(e, 'X'), alert('X')
    for (const fn of MESSAGE_CALLS) {
      const re = new RegExp(`\\b${fn}\\(([^)]*?)\\)`, 'g');
      for (const m of line.matchAll(re)) {
        const args = m[1];
        for (const sm of args.matchAll(/(['"])((?:\\.|(?!\1).)+)\1/g)) {
          const txt = cleanText(sm[2]);
          if (isCopyText(txt) && !args.includes('t(')) {
            findings.push({ file, line: i + 1, kind: `call:${fn}`, text: txt });
          }
        }
      }
    }
    // 5. Locale-pinned formatters
    if (/toLocale(String|DateString|TimeString)\(\s*['"]|Intl\.(DateTimeFormat|NumberFormat|RelativeTimeFormat)\(\s*['"]/.test(line)) {
      findings.push({ file, line: i + 1, kind: 'locale-pinned-format', text: line.trim().slice(0, 120) });
    }
    // 6. {'literal'} inside JSX expression containers
    for (const m of line.matchAll(/\{(['"])((?:\\.|(?!\1).)*[A-Za-z](?:\\.|(?!\1).)*)\1\}/g)) {
      const txt = cleanText(m[2]);
      if (isCopyText(txt) && !/^(t|i18n)\s*\(/.test(line.slice(Math.max(0, m.index - 4), m.index))) {
        findings.push({ file, line: i + 1, kind: 'jsx-expr', text: txt });
      }
    }
  }
  return findings;
}

// Same unescaping the generator applies: backslash-newline continuation,
// \n, \" and \\, then whitespace collapse — the runtime form of the literal.
const normalizeRust = (s) =>
  s
    .replace(/\\\n\s*/g, ' ')
    .replace(/\\n/g, ' ')
    .replace(/\\"/g, '"')
    .replace(/\\\\/g, '\\')
    .replace(/\s+/g, ' ')
    .trim();

function scanBackend() {
  const messages = new Set();
  const re = /AppError::(BadRequest|Unauthorized|Forbidden|NotFound|Conflict|Validation|Internal)\(\s*"((?:[^"\\]|\\[\s\S])*)"/g;
  for (const file of walk(BE_SRC)) {
    if (!file.endsWith('.rs') || file.includes('/tests/')) continue;
    const src = readFileSync(file, 'utf8');
    for (const m of src.matchAll(re)) {
      messages.add(normalizeRust(m[2]));
    }
  }
  return [...messages].sort();
}

function serverErrorMapKeys() {
  const f = join(FE_SRC, 'i18n', 'serverErrorMap.ts');
  const src = readFileSync(f, 'utf8');
  const keys = new Set();
  for (const m of src.matchAll(/^\s*'((?:\\.|[^'])+)':/gm)) {
    keys.add(m[1].replace(/\\'/g, "'"));
  }
  return keys;
}

// --- run -----------------------------------------------------------------
const feFindings = [];
for (const file of walk(FE_SRC)) {
  if (!/\.(ts|tsx)$/.test(file)) continue;
  if (SKIP_FILES.has(file.split('/').pop())) continue;
  feFindings.push(...scanFrontendFile(file));
}

const beMessages = scanBackend();
const mapped = serverErrorMapKeys();
const beUnmapped = beMessages.filter((m) => !mapped.has(m));

const report = {
  frontend: {
    findings: feFindings.length,
    byKind: Object.fromEntries(
      [...new Set(feFindings.map(f => f.kind))].sort().map(k => [k, feFindings.filter(f => f.kind === k).length])
    ),
    items: feFindings.map(f => ({ ...f, file: relative(ROOT, f.file) })),
  },
  backend: {
    totalMessages: beMessages.length,
    mapped: beMessages.length - beUnmapped.length,
    unmapped: beUnmapped,
  },
};

const json = process.argv.includes('--json');
if (json) {
  console.log(JSON.stringify(report, null, 2));
} else {
  console.log(`frontend findings: ${report.frontend.findings}`);
  for (const [kind, n] of Object.entries(report.frontend.byKind)) {
    console.log(`  ${kind}: ${n}`);
  }
  console.log(`backend messages: ${report.backend.totalMessages} total, ${report.backend.mapped} mapped, ${beUnmapped.length} unmapped`);
  for (const f of report.frontend.items.slice(0, 5000)) {
    console.log(`  ${f.file}:${f.line} [${f.kind}] ${f.text}`);
  }
  if (beUnmapped.length) {
    console.log('\nunmapped backend messages:');
    for (const m of beUnmapped.slice(0, 100)) console.log(`  ${m}`);
  }
}

process.exit(process.argv.includes('--ci') && feFindings.length > 0 ? 1 : 0);
