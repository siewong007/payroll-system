import { QrCode, Fingerprint, User } from 'lucide-react';
import i18n, { intlLocale } from '@/i18n';
import {
  formatTime as formatTimeL,
  formatDateMedium,
  formatDateTime as formatDateTimeL,
} from '@/lib/format';

const zoneDateFormatters = new Map<string, Intl.DateTimeFormat>();

function getZoneDateFormatter(timeZone?: string): Intl.DateTimeFormat {
  const key = timeZone ?? '';
  let formatter = zoneDateFormatters.get(key);
  if (!formatter) {
    // i18n-ok: en-CA produces ISO YYYY-MM-DD for API params, not display.
    formatter = new Intl.DateTimeFormat(
      'en-CA',
      timeZone ? { timeZone, year: 'numeric', month: '2-digit', day: '2-digit' } : undefined,
    );
    zoneDateFormatters.set(key, formatter);
  }
  return formatter;
}

/**
 * Today's date on the *company* calendar, not the viewer's. Attendance days
 * are bucketed in the company timezone server-side, so a viewer in another
 * zone (or near midnight) would otherwise label the wrong day "today".
 */
export function todayInZone(timeZone: string | undefined): string {
  try {
    return getZoneDateFormatter(timeZone || 'Asia/Kuala_Lumpur').format(new Date());
  } catch {
    return getZoneDateFormatter().format(new Date());
  }
}

export function firstOfMonth(day: string): string {
  return `${day.slice(0, 7)}-01`;
}

export const STATUS_COLORS = {
  present: 'bg-emerald-100 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-300',
  late: 'bg-amber-100 text-amber-700 dark:bg-amber-500/20 dark:text-amber-300',
  absent: 'bg-red-100 text-red-700 dark:bg-red-500/20 dark:text-red-300',
  half_day: 'bg-blue-100 text-blue-700 dark:bg-blue-500/20 dark:text-blue-300',
} as const;

export const METHOD_CONFIG = {
  qr_code: { icon: QrCode, color: 'text-violet-600 dark:text-violet-400' },
  face_id: { icon: Fingerprint, color: 'text-sky-600 dark:text-sky-400' },
  manual: { icon: User, color: 'text-gray-500 dark:text-slate-400' },
} as const;

export function attStatusLabel(status: string): string {
  return i18n.t(`enums.attendanceStatus.${status}`, { defaultValue: status });
}

export function attMethodLabel(method: string): string {
  return i18n.t(`enums.attendanceMethod.${method}`, { defaultValue: method });
}

export function formatTime(iso: string | null, timeZone?: string) {
  if (!iso) return '—';
  if (!timeZone) return formatTimeL(iso);
  try {
    return new Intl.DateTimeFormat(intlLocale(), {
      timeZone,
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(iso));
  } catch {
    return formatTimeL(iso);
  }
}

export function formatDate(iso: string, timeZone?: string) {
  if (!timeZone) return formatDateMedium(iso);
  try {
    return new Intl.DateTimeFormat(intlLocale(), {
      timeZone,
      day: 'numeric',
      month: 'short',
      year: 'numeric',
    }).format(new Date(iso));
  } catch {
    return formatDateMedium(iso);
  }
}

export function formatDateTime(iso: string | null) {
  return iso ? formatDateTimeL(iso) : '—';
}
