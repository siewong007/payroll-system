import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import i18n from '@/i18n';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { formatDate, formatDateMedium, formatDateTime, weekdayName } from '@/lib/format';
import { calculateOvertimeHours } from '@/lib/overtime';
import { getErrorMessage } from '@/lib/utils';

describe('Audit regressions — format helpers', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('parses YYYY-MM-DD as a local calendar date without shifting backward across timezones', () => {
    expect(formatDate('2026-03-01')).toMatch(/0?1/);
    expect(formatDateMedium('2026-03-01')).toContain('Mar');
    expect(formatDateMedium('2026-03-01')).toContain('2026');
  });

  it('returns an empty string instead of throwing on invalid date values', () => {
    expect(formatDate('not-a-date')).toBe('');
    expect(formatDate('2026-02-30')).toBe('');
    expect(formatDateTime('')).toBe('');
  });

  it('renders 7 distinct narrow weekday headers in Chinese locales', async () => {
    for (const lang of ['zh-CN', 'zh-TW'] as const) {
      await i18n.changeLanguage(lang);
      const headers = [0, 1, 2, 3, 4, 5, 6].map((dow) => weekdayName(dow, 'narrow'));
      expect(new Set(headers).size).toBe(7);
    }
  });
});

describe('Audit regressions — overtime calculation', () => {
  it('computes exact decimal hours instead of rounding non-30m windows above elapsed time', () => {
    expect(calculateOvertimeHours('17:00', '18:45')).toBe(1.75);
    expect(calculateOvertimeHours('23:00', '00:45')).toBe(1.75);
  });
});

describe('Audit regressions — DataTable', () => {
  interface KeylessRow {
    period: string;
    total: number;
  }

  const columns: Column<KeylessRow>[] = [
    { key: 'period', header: 'Period', render: (r) => r.period, primary: true },
    { key: 'total', header: 'Total', render: (r) => String(r.total) },
  ];

  it('keeps the summary modal open when rows have no .id property and no rowKey prop', async () => {
    const user = userEvent.setup();
    render(
      <DataTable
        columns={columns}
        data={[
          { period: '2026-01', total: 100 },
          { period: '2026-02', total: 200 },
        ]}
      />,
    );

    const tbody = screen.getByRole('table').querySelector('tbody') as HTMLElement;
    const rows = within(tbody).getAllByRole('row');
    await user.click(rows[0]);

    await waitFor(() => expect(screen.getByRole('dialog')).toBeInTheDocument());
    expect(within(screen.getByRole('dialog')).getByText('2026-01')).toBeInTheDocument();
  });

  it('uses pre-filter rowKey indices when selecting all selectable rows', async () => {
    const user = userEvent.setup();
    const onSelectionChange = vi.fn();

    function Harness() {
      const [selected, setSelected] = useState<string[]>([]);
      return (
        <DataTable
          columns={columns}
          data={[
            { period: 'skip-0', total: 1 },
            { period: 'keep-1', total: 2 },
            { period: 'keep-2', total: 3 },
          ]}
          rowKey={(_row, index) => `row-${index}`}
          selectable
          selectedRowKeys={selected}
          onSelectedRowKeysChange={(keys) => {
            setSelected(keys);
            onSelectionChange(keys);
          }}
          isRowSelectable={(row) => row.period !== 'skip-0'}
        />
      );
    }

    render(<Harness />);
    const headerCheckbox = within(screen.getByRole('table')).getAllByRole('checkbox')[0];
    await user.click(headerCheckbox);

    expect(onSelectionChange).toHaveBeenLastCalledWith(['row-1', 'row-2']);
  });
});

describe('Audit regressions — getErrorMessage 429 handling', () => {
  it('prefers the translated server error message over generic 429 prose', () => {
    const err = {
      response: {
        status: 429,
        data: { error: 'Too many failed 2FA attempts. Please try again later.' },
      },
    };
    expect(getErrorMessage(err, 'Fallback')).toBe(
      'Too many failed 2FA attempts. Please try again later.',
    );
  });
});
