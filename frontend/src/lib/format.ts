import i18n, { intlLocale } from '@/i18n';

/**
 * Locale-aware formatting. Every helper reads the active i18n language at call
 * time, so components re-rendered by a language switch pick up the new locale
 * without plumbing. Currency stays MYR (the transaction currency) — only the
 * digits and grouping follow the UI locale.
 */

const dateTimeFormatters = new Map<string, Intl.DateTimeFormat>();
const numberFormatters = new Map<string, Intl.NumberFormat>();

function getDateFormatter(
  locale: string,
  key: string,
  options: Intl.DateTimeFormatOptions,
): Intl.DateTimeFormat {
  const cacheKey = `${locale}|${key}`;
  let formatter = dateTimeFormatters.get(cacheKey);
  if (!formatter) {
    formatter = new Intl.DateTimeFormat(locale, options);
    dateTimeFormatters.set(cacheKey, formatter);
  }
  return formatter;
}

function getNumberFormatter(
  locale: string,
  key: string,
  options?: Intl.NumberFormatOptions,
): Intl.NumberFormat {
  const cacheKey = `${locale}|${key}`;
  let formatter = numberFormatters.get(cacheKey);
  if (!formatter) {
    formatter = new Intl.NumberFormat(locale, options);
    numberFormatters.set(cacheKey, formatter);
  }
  return formatter;
}

const DATE_SHORT: Intl.DateTimeFormatOptions = {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
};

const DATE_LONG: Intl.DateTimeFormatOptions = {
  day: 'numeric',
  month: 'long',
  year: 'numeric',
};

const DATE_MEDIUM: Intl.DateTimeFormatOptions = {
  day: 'numeric',
  month: 'short',
  year: 'numeric',
};

const DATE_DAY_MONTH: Intl.DateTimeFormatOptions = {
  day: 'numeric',
  month: 'short',
};

const DATE_WEEKDAY_LONG: Intl.DateTimeFormatOptions = {
  weekday: 'long',
  day: 'numeric',
  month: 'long',
};

const DATETIME: Intl.DateTimeFormatOptions = {
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
};

const TIME: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit' };

const TIME_SECONDS: Intl.DateTimeFormatOptions = {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
};

const TIME_HOUR: Intl.DateTimeFormatOptions = { hour: 'numeric', minute: '2-digit' };

const MYR_OPTIONS: Intl.NumberFormatOptions = {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
};

const DATE_ONLY_RE = /^(\d{4})-(\d{2})-(\d{2})$/;

function toDate(value: string | Date): Date | null {
  if (value instanceof Date) {
    return Number.isNaN(value.getTime()) ? null : value;
  }
  const trimmed = value.trim();
  if (!trimmed) return null;
  const dateOnly = DATE_ONLY_RE.exec(trimmed);
  if (dateOnly) {
    const year = Number(dateOnly[1]);
    const month = Number(dateOnly[2]);
    const day = Number(dateOnly[3]);
    const parsed = new Date(year, month - 1, day);
    if (
      parsed.getFullYear() !== year ||
      parsed.getMonth() !== month - 1 ||
      parsed.getDate() !== day
    ) {
      return null;
    }
    return parsed;
  }
  const parsed = new Date(trimmed);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function formatWith(value: string | Date, key: string, options: Intl.DateTimeFormatOptions): string {
  const date = toDate(value);
  if (!date) return '';
  return getDateFormatter(intlLocale(), key, options).format(date);
}

/** DD/MM/YYYY-style short date (locale-ordered). */
export function formatDate(value: string | Date): string {
  return formatWith(value, 'short', DATE_SHORT);
}

/** "16 September 2026" — locale long form. */
export function formatDateLong(value: string | Date): string {
  return formatWith(value, 'long', DATE_LONG);
}

/** "16 Sep 2026" — locale medium form. */
export function formatDateMedium(value: string | Date): string {
  return formatWith(value, 'medium', DATE_MEDIUM);
}

/** "16 Sep" — no year, for compact ranges. */
export function formatDateDayMonth(value: string | Date): string {
  return formatWith(value, 'dayMonth', DATE_DAY_MONTH);
}

/** "Wednesday, 16 September". */
export function formatDateWeekday(value: string | Date): string {
  return formatWith(value, 'weekdayLong', DATE_WEEKDAY_LONG);
}

/** Date + time, e.g. "16 Sep 2026, 02:30 PM". */
export function formatDateTime(value: string | Date): string {
  return formatWith(value, 'datetime', DATETIME);
}

/** "02:30 PM" / "14:30" depending on locale conventions. */
export function formatTime(value: string | Date): string {
  return formatWith(value, 'time', TIME);
}

export function formatTimeSeconds(value: string | Date): string {
  return formatWith(value, 'timeSec', TIME_SECONDS);
}

/** Non-padded hour form ("2:30 PM") used by attendance displays. */
export function formatTimeHour(value: string | Date): string {
  return formatWith(value, 'timeHour', TIME_HOUR);
}

/** Grouped number, no decimals: 1,234 / 1.234 / 1,234 per locale. */
export function formatNumber(value: number): string {
  return getNumberFormatter(intlLocale(), 'int').format(value);
}

/** Localized month name for a 1-12 month number ("September" / "九月"). */
export function monthName(month: number, style: 'short' | 'long' = 'long', locale?: string): string {
  return getDateFormatter(intlLocale(locale), `month:${style}`, { month: style }).format(
    new Date(2000, month - 1, 1),
  );
}

/** Localized weekday name for a JS day-of-week (0 = Sunday). */
export function weekdayName(
  day: number,
  style: 'short' | 'long' | 'narrow' = 'short',
  locale?: string,
): string {
  // 2000-01-02 was a Sunday, so day N lands on weekday N.
  return getDateFormatter(intlLocale(locale), `weekday:${style}`, { weekday: style }).format(
    new Date(2000, 0, 2 + day),
  );
}

/** "September 2026" / "2026年9月" — a payroll period (month + year). */
export function formatPeriod(year: number, month: number): string {
  return i18n.t('payroll.period', {
    year,
    month: monthName(month),
    monthNum: month,
  });
}

/** Sen → "RM 1,234.56". The currency is fixed MYR; only formatting localizes. */
export function formatMYR(sen: number): string {
  const ringgit = sen / 100;
  return `RM ${getNumberFormatter(intlLocale(), 'myr', MYR_OPTIONS).format(ringgit)}`;
}

/**
 * "just now", "5 min ago", "3 hr ago", "2 days ago", then the absolute date —
 * translated via the `time.*` keys (the terse copy is deliberate UI style,
 * not a truncation of RelativeTimeFormat's "5 minutes ago").
 */
export function formatRelativeTime(value: string | Date): string {
  const date = toDate(value);
  if (!date) return '';
  const seconds = Math.max(0, Math.floor((Date.now() - date.getTime()) / 1000));
  if (seconds < 60) return i18n.t('time.justNow');
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return i18n.t('time.minutesAgo', { count: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return i18n.t('time.hoursAgo', { count: hours });
  const days = Math.floor(hours / 24);
  if (days < 30) return i18n.t('time.daysAgo', { count: days });
  return formatDate(value);
}
