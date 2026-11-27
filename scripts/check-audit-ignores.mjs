#!/usr/bin/env node
/**
 * Machine-enforced audit ignore expiry & registry sync gate (plan item 49).
 *
 * Ensures:
 *   1. Every security ignore in `.github/workflows/ci.yml` and `backend/deny.toml`
 *      is registered in `.github/audit-ignores.json` with a non-empty reason
 *      and an `YYYY-MM-DD` expiry date.
 *   2. Every registered entry has not expired (using `AUDIT_CHECK_DATE` or today UTC)
 *      and does not exceed 180 days from today (preventing indefinite suppressions).
 *   3. No stale registry entries linger after an ignore is removed from CI.
 */
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(fileURLToPath(new URL('.', import.meta.url)), '..');
const REGISTRY_PATH = join(ROOT, '.github', 'audit-ignores.json');
const CI_PATH = join(ROOT, '.github', 'workflows', 'ci.yml');
const DENY_PATH = join(ROOT, 'backend', 'deny.toml');

const MAX_HORIZON_DAYS = 180;
const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

function todayIso() {
  const override = process.env.AUDIT_CHECK_DATE;
  if (override) {
    if (!DATE_RE.test(override)) {
      throw new Error(`Invalid AUDIT_CHECK_DATE "${override}", expected YYYY-MM-DD`);
    }
    return override;
  }
  return new Date().toISOString().slice(0, 10);
}

function daysBetween(fromIso, toIso) {
  const fromMs = Date.parse(`${fromIso}T00:00:00Z`);
  const toMs = Date.parse(`${toIso}T00:00:00Z`);
  return Math.round((toMs - fromMs) / 86_400_000);
}

const registry = JSON.parse(readFileSync(REGISTRY_PATH, 'utf8'));
if (!Array.isArray(registry.ignores)) {
  throw new Error('.github/audit-ignores.json must contain an "ignores" array');
}

const today = todayIso();
const errors = [];
const registeredIds = new Set();

for (const entry of registry.ignores) {
  if (!entry.id || typeof entry.id !== 'string') {
    errors.push('Registry entry is missing string "id"');
    continue;
  }
  registeredIds.add(entry.id);
  if (!['cargo', 'bun'].includes(entry.ecosystem)) {
    errors.push(`${entry.id}: invalid ecosystem "${entry.ecosystem}"`);
  }
  if (!entry.reason || typeof entry.reason !== 'string' || entry.reason.trim().length < 10) {
    errors.push(`${entry.id}: must include a descriptive "reason" (>= 10 chars)`);
  }
  if (!entry.expires || !DATE_RE.test(entry.expires) || Number.isNaN(Date.parse(`${entry.expires}T00:00:00Z`))) {
    errors.push(`${entry.id}: invalid "expires" date "${entry.expires}" (expected YYYY-MM-DD)`);
    continue;
  }

  if (entry.ecosystem === 'cargo') {
    if (typeof entry.compiled !== 'boolean') {
      errors.push(`${entry.id}: cargo ignore entries must explicitly set boolean "compiled"`);
    } else if (entry.compiled === false && entry.package) {
      try {
        const treeOut = execFileSync(
          'cargo',
          ['tree', '--manifest-path', join(ROOT, 'backend', 'Cargo.toml'), '--target', 'all', '-i', entry.package],
          { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
        );
        if (treeOut.includes(entry.package)) {
          errors.push(
            `${entry.id}: marked compiled=false for "${entry.package}", but cargo tree shows it in the active dependency graph`,
          );
        }
      } catch (err) {
        if (err && typeof err === 'object' && 'code' in err && err.code !== 'ENOENT') {
          const msg = err instanceof Error ? err.message : String(err);
          errors.push(`${entry.id}: failed to verify cargo dependency graph for "${entry.package}": ${msg}`);
        }
      }
    }
  }

  const remainingDays = daysBetween(today, entry.expires);
  if (remainingDays < 0) {
    errors.push(`${entry.id} (${entry.ecosystem}/${entry.package}): ignore EXPIRED on ${entry.expires} (today ${today})`);
  } else if (remainingDays > MAX_HORIZON_DAYS) {
    errors.push(
      `${entry.id}: expiry ${entry.expires} is ${remainingDays} days away (max ${MAX_HORIZON_DAYS} days)`,
    );
  }
}

const ciYaml = readFileSync(CI_PATH, 'utf8');
const ciIds = new Set();
for (const match of ciYaml.matchAll(/\bignore:\s*([^\n#]+)/g)) {
  for (const token of match[1].split(',')) {
    const id = token.trim();
    if (/^(RUSTSEC|GHSA)-/.test(id)) ciIds.add(id);
  }
}
for (const match of ciYaml.matchAll(/--ignore\s+([A-Za-z0-9_-]+)/g)) {
  if (/^(RUSTSEC|GHSA)-/.test(match[1])) ciIds.add(match[1]);
}

const denyToml = readFileSync(DENY_PATH, 'utf8');
const denyIds = new Set();
for (const match of denyToml.matchAll(/"(RUSTSEC-\d{4}-\d+)"/g)) {
  denyIds.add(match[1]);
}

for (const id of ciIds) {
  if (!registeredIds.has(id)) {
    errors.push(`ci.yml suppresses ${id}, but it is missing from .github/audit-ignores.json`);
  }
}
for (const id of denyIds) {
  if (!registeredIds.has(id)) {
    errors.push(`backend/deny.toml suppresses ${id}, but it is missing from .github/audit-ignores.json`);
  }
}
for (const id of registeredIds) {
  if (!ciIds.has(id)) {
    errors.push(`.github/audit-ignores.json lists ${id}, which is no longer suppressed in ci.yml`);
  }
}

if (errors.length > 0) {
  console.error('Audit ignore policy check failed:');
  for (const err of errors) {
    console.error(`  - ${err}`);
  }
  process.exit(1);
}

console.log(
  `Audit ignore policy OK (${registry.ignores.length} active entries verified against ${today}).`,
);
