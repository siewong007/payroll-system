import { describe, expect, it } from 'vitest';
import { cn, formatDate, formatMYR, getErrorMessage } from '@/lib/utils';

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
