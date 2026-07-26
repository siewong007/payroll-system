import { describe, expect, it } from 'vitest';
import {
  cn,
  formatDate,
  formatMYR,
  getErrorMessage,
  toDateTimeLocalValue,
  safeRedirectPath,
  todayLocalDate,
} from '@/lib/utils';

describe('shared utilities', () => {
  it('combines conditional class names', () => {
    expect(cn('base', false, undefined, { active: true, hidden: false }, ['nested']))
      .toBe('base active nested');
  });

  it('formats sen as Malaysian ringgit including negatives and zero', () => {
    expect(formatMYR(123456)).toBe('RM 1,234.56');
    expect(formatMYR(-50)).toBe('RM -0.50');
    expect(formatMYR(0)).toBe('RM 0.00');
  });

  it('formats a local calendar date as DD/MM/YYYY', () => {
    expect(formatDate(new Date(2026, 6, 14, 12, 0, 0))).toBe('14/07/2026');
  });

  it('prefers an API error, then a standard Error, then the supplied fallback', () => {
    expect(getErrorMessage({ response: { data: { error: 'Employee number already exists' } } }))
      .toBe('Employee number already exists');
    expect(getErrorMessage(new Error('Network unavailable'))).toBe('Network unavailable');
    expect(getErrorMessage('unexpected', 'Could not save employee')).toBe('Could not save employee');
    expect(getErrorMessage({ response: { data: {} } })).toBe('Action failed');
  });
});

describe('datetime-local round trip', () => {
  // The attendance edit modal fills a datetime-local input from a stored instant
  // and submits it back with new Date(value).toISOString(). If the two halves
  // disagree on timezone, opening and saving an untouched record shifts the
  // stored time by the UTC offset on every save.
  it('round-trips an instant through the input value without drifting', () => {
    const stored = '2026-07-26T01:30:00Z';
    const inputValue = toDateTimeLocalValue(stored);
    const submitted = new Date(inputValue).toISOString();
    expect(new Date(submitted).getTime()).toBe(new Date(stored).getTime());
  });

  it('survives repeated open-and-save cycles', () => {
    let current = '2026-07-26T01:30:00Z';
    for (let i = 0; i < 5; i += 1) {
      current = new Date(toDateTimeLocalValue(current)).toISOString();
    }
    expect(new Date(current).getTime()).toBe(new Date('2026-07-26T01:30:00Z').getTime());
  });

  it('formats local wall time, not UTC wall time', () => {
    const d = new Date(2026, 6, 26, 8, 5);
    expect(toDateTimeLocalValue(d)).toBe('2026-07-26T08:05');
  });

  it('reports the local calendar day', () => {
    expect(todayLocalDate()).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(todayLocalDate()).toBe(toDateTimeLocalValue(new Date()).slice(0, 10));
  });
});

describe('safeRedirectPath', () => {
  it('accepts a same-origin path with its query string', () => {
    expect(safeRedirectPath('/attendance/scan?token=abc')).toBe('/attendance/scan?token=abc');
  });

  it('rejects protocol-relative and absolute targets', () => {
    // '//evil.com' is treated as an absolute URL by the browser — the classic
    // open-redirect bypass of a naive startsWith('/') check.
    expect(safeRedirectPath('//evil.com')).toBeNull();
    expect(safeRedirectPath('https://evil.com')).toBeNull();
    expect(safeRedirectPath('http://evil.com')).toBeNull();
    expect(safeRedirectPath('javascript:alert(1)')).toBeNull();
  });

  it('rejects empty and missing values', () => {
    expect(safeRedirectPath(null)).toBeNull();
    expect(safeRedirectPath(undefined)).toBeNull();
    expect(safeRedirectPath('')).toBeNull();
    expect(safeRedirectPath('portal')).toBeNull();
  });
});
