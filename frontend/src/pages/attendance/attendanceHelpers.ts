import { QrCode, Fingerprint, User } from 'lucide-react';
import i18n from '@/i18n';
import {
  formatTime as formatTimeL,
  formatDateMedium,
  formatDateTime as formatDateTimeL,
} from '@/lib/format';

/**
 * Today's date on the *company* calendar, not the viewer's. Attendance days
 * are bucketed in the company timezone server-side, so a viewer in another
 * zone (or near midnight) would otherwise label the wrong day "today".
 */
export function todayInZone(timeZone: string | undefined): string {
  try {
    // i18n-ok: en-CA produces ISO YYYY-MM-DD for API params, not display.
    return new Intl.DateTimeFormat('en-CA', {
      timeZone: timeZone || 'Asia/Kuala_Lumpur',
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
    }).format(new Date());
  } catch {
    return new Intl.DateTimeFormat('en-CA').format(new Date()); // i18n-ok
  }
}

export function firstOfMonth(day: string): string {
  return `${day.slice(0, 7)}-01`;
}

export const STATUS_COLORS = {
  present: 'bg-emerald-100 text-emerald-700',
  late: 'bg-amber-100  text-amber-700',
  absent: 'bg-red-100    text-red-700',
  half_day: 'bg-blue-100   text-blue-700',
} as const;

export const METHOD_CONFIG = {
  qr_code: { icon: QrCode, color: 'text-violet-600' },
  face_id: { icon: Fingerprint, color: 'text-sky-600' },
  manual: { icon: User, color: 'text-gray-500' },
} as const;

export function attStatusLabel(status: string): string {
  return i18n.t(`enums.attendanceStatus.${status}`, { defaultValue: status });
}

export function attMethodLabel(method: string): string {
  return i18n.t(`enums.attendanceMethod.${method}`, { defaultValue: method });
}

export function formatTime(iso: string | null) {
  return iso ? formatTimeL(iso) : '—';
}

export function formatDate(iso: string) {
  return formatDateMedium(iso);
}

export function formatDateTime(iso: string | null) {
  return iso ? formatDateTimeL(iso) : '—';
}
