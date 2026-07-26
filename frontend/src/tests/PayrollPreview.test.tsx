import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { PayrollPreviewPanel } from '@/pages/payroll/PayrollPreviewPanel';
import type { PayrollPreview, PayrollPreviewEmployee } from '@/types';

function employee(overrides: Partial<PayrollPreviewEmployee> = {}): PayrollPreviewEmployee {
  return {
    employee_id: 'emp-1',
    employee_name: 'Aisyah Rahman',
    employee_number: 'E001',
    basic_salary: 500_000,
    total_allowances: 30_000,
    total_overtime: 0,
    total_claims: 0,
    gross_salary: 530_000,
    epf_employee: 53_000,
    socso_employee: 1_825,
    eis_employee: 990,
    pcb_amount: 12_300,
    total_deductions: 68_115,
    net_salary: 461_885,
    employer_cost: 592_325,
    working_days: 31,
    days_worked: 31,
    is_prorated: false,
    error: null,
    ...overrides,
  };
}

function preview(overrides: Partial<PayrollPreview> = {}): PayrollPreview {
  return {
    payroll_group_id: 'group-1',
    period_year: 2026,
    period_month: 7,
    period_start: '2026-07-01',
    period_end: '2026-07-31',
    pay_date: '2026-07-28',
    employee_count: 1,
    payable_count: 1,
    total_gross: 530_000,
    total_net: 461_885,
    total_employer_cost: 592_325,
    total_epf_employee: 53_000,
    total_epf_employer: 58_000,
    total_socso_employee: 1_825,
    total_socso_employer: 3_335,
    total_eis_employee: 990,
    total_eis_employer: 990,
    total_pcb: 12_300,
    total_zakat: 0,
    can_process: true,
    blocking: [],
    warnings: [],
    employees: [employee()],
    ...overrides,
  };
}

describe('PayrollPreviewPanel', () => {
  it('lets a clean run be confirmed and states that nothing is saved yet', async () => {
    const user = userEvent.setup();
    const onProcess = vi.fn();

    render(
      <PayrollPreviewPanel
        preview={preview()}
        onProcess={onProcess}
        onBack={vi.fn()}
        isProcessing={false}
      />,
    );

    expect(screen.getByText(/Ready to process 1 payslip/)).toBeInTheDocument();
    expect(screen.getByText(/Nothing has been saved/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /Confirm & Process Payroll/ }));
    expect(onProcess).toHaveBeenCalledTimes(1);
  });

  it('blocks processing and lists every failing employee at once', () => {
    const onProcess = vi.fn();

    render(
      <PayrollPreviewPanel
        preview={preview({
          can_process: false,
          employee_count: 3,
          payable_count: 1,
          blocking: [
            {
              code: 'employee_calculation_failed',
              message: 'Deductions exceed gross plus claims.',
              employee_id: 'emp-2',
              employee_number: 'E002',
              employee_name: 'Chandran Nair',
            },
            {
              code: 'employee_calculation_failed',
              message: 'No verified EPF band for this wage.',
              employee_id: 'emp-3',
              employee_number: 'E003',
              employee_name: 'Lim Wei Han',
            },
          ],
          employees: [
            employee(),
            employee({
              employee_id: 'emp-2',
              employee_number: 'E002',
              employee_name: 'Chandran Nair',
              error: 'Deductions exceed gross plus claims.',
            }),
          ],
        })}
        onProcess={onProcess}
        onBack={vi.fn()}
        isProcessing={false}
      />,
    );

    expect(screen.getByRole('button', { name: /Confirm & Process Payroll/ })).toBeDisabled();
    expect(screen.getByText(/Must be fixed before processing \(2\)/)).toBeInTheDocument();
    // Both failures are visible together rather than one run at a time.
    expect(screen.getByText(/No verified EPF band for this wage/)).toBeInTheDocument();
    expect(screen.getByText(/2 of 3 employees could not be calculated/)).toBeInTheDocument();
    expect(screen.getByText('Not calculable')).toBeInTheDocument();
  });

  it('shows warnings without blocking the run', () => {
    render(
      <PayrollPreviewPanel
        preview={preview({
          warnings: [
            {
              code: 'staged_entries_not_in_run',
              message: '1 staged entry will not be paid by this run.',
              employee_id: 'emp-9',
              employee_number: 'E009',
              employee_name: 'Nurul Huda',
            },
          ],
        })}
        onProcess={vi.fn()}
        onBack={vi.fn()}
        isProcessing={false}
      />,
    );

    expect(screen.getByText(/Worth reviewing \(1\)/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Confirm & Process Payroll/ })).toBeEnabled();
  });

  it('flags a prorated employee so a short payslip is not mistaken for an error', () => {
    render(
      <PayrollPreviewPanel
        preview={preview({
          employees: [employee({ is_prorated: true, days_worked: 12, working_days: 31 })],
        })}
        onProcess={vi.fn()}
        onBack={vi.fn()}
        isProcessing={false}
      />,
    );

    expect(screen.getByText('Prorated 12/31 days')).toBeInTheDocument();
  });
});
