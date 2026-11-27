import i18n from '@/i18n';
import { formatDate, formatMYR, formatPeriod } from '@/lib/format';
import { translateServerMessage } from '@/lib/utils';
import type { PayrollDiagnostic } from '@/types';

/// Payroll-run status display metadata, shared by the runs list, the detail
/// page and the overview. Lives outside the page modules because lazyNamed()
/// requires every export of a lazily imported module to be a component.
export const PAYROLL_STATUS_STYLES: Record<string, string> = {
  draft: 'bg-zinc-100 text-zinc-700 border border-zinc-200 dark:bg-zinc-800 dark:text-zinc-300 dark:border-zinc-700',
  processing: 'bg-amber-50 text-amber-800 border border-amber-200 dark:bg-amber-500/12 dark:text-amber-300 dark:border-amber-500/25',
  processed: 'bg-zinc-100 text-zinc-900 border border-zinc-300 dark:bg-zinc-800 dark:text-zinc-100 dark:border-zinc-600',
  pending_approval: 'bg-blue-50 text-blue-800 border border-blue-200 dark:bg-blue-500/12 dark:text-blue-300 dark:border-blue-500/25',
  approved: 'bg-emerald-50 text-emerald-800 border border-emerald-200 dark:bg-emerald-500/12 dark:text-emerald-300 dark:border-emerald-500/25',
  paid: 'bg-emerald-50 text-emerald-800 border border-emerald-200 dark:bg-emerald-500/12 dark:text-emerald-300 dark:border-emerald-500/25',
  cancelled: 'bg-red-50 text-red-800 border border-red-200 dark:bg-red-500/12 dark:text-red-300 dark:border-red-500/25',
};

export const PAYROLL_STATUSES = [
  'draft',
  'processing',
  'processed',
  'pending_approval',
  'approved',
  'paid',
  'cancelled',
] as const;

/** Localized label for a payroll-run status; falls back to the raw status. */
export function payrollStatusLabel(status: string): string {
  return i18n.t(`enums.payrollStatus.${status}`, { defaultValue: status });
}

const MM_YYYY = /^(\d{2})\/(\d{4})$/;

/**
 * Localized text for a backend `PayrollDiagnostic`. Codes with a translation
 * key render from `payroll.diagnostics.<code>` using the structured `params`
 * payload (sen totals are formatted as MYR, `MM/YYYY` period strings as
 * localized periods, `oldest` as a localized date); anything without a key —
 * or a diagnostic carrying no params — falls back to the server `message`
 * passed through the server-error map.
 */
export function payrollDiagnosticText(d: PayrollDiagnostic): string {
  const key = `payroll.diagnostics.${d.code}`;
  const vars: Record<string, string | number> = {};
  if (d.params) {
    for (const [k, v] of Object.entries(d.params)) {
      const period = v.match(MM_YYYY);
      if ((k === 'total' || k === 'salary' || k === 'minimum') && /^\d+$/.test(v)) {
        vars[k] = formatMYR(Number(v));
      } else if (k === 'oldest' && /^\d{4}-\d{2}-\d{2}$/.test(v)) {
        vars[k] = formatDate(new Date(`${v}T00:00:00`));
      } else if (period) {
        vars[k] = formatPeriod(Number(period[2]), Number(period[1]));
      } else if (k === 'periods') {
        vars[k] = v
          .split(', ')
          .map((p) => {
            const m = p.match(MM_YYYY);
            return m ? formatPeriod(Number(m[2]), Number(m[1])) : p;
          })
          .join(', ');
      } else {
        vars[k] = v;
      }
    }
    if (vars.count !== undefined) vars.count = Number(vars.count);
  }
  const exists =
    vars.count !== undefined
      ? i18n.exists(key, { count: Number(vars.count) })
      : i18n.exists(key);
  if (!exists) return translateServerMessage(d.message);
  return i18n.t(key, vars);
}
