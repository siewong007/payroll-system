import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ToastProvider } from '@/components/ui/Toast';
import { ConfirmProvider } from '@/components/ui/ConfirmDialog';
import { PromptProvider } from '@/components/ui/PromptDialog';
import { Modal } from '@/components/ui/Modal';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { ThemeProvider } from '@/context/ThemeProvider';
import { AppearanceSwitcher } from '@/components/ui/AppearanceSwitcher';
import { CommandPalette } from '@/components/ui/CommandPalette';
import { toast, useToast } from '@/lib/toast';
import { useConfirm } from '@/lib/confirm';
import { usePrompt } from '@/lib/prompt';
import { openCommandPalette } from '@/lib/commandPalette';
import { getErrorMessage } from '@/lib/utils';
import type { AppRole, DashboardSummary, User } from '@/types';
import { userWithRoles } from './support/permissions';

const authMocks = vi.hoisted(() => ({ useAuth: vi.fn() }));
const dashboardMocks = vi.hoisted(() => ({ getDashboardSummary: vi.fn() }));
const employeeMocks = vi.hoisted(() => ({ getEmployees: vi.fn() }));

vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));
vi.mock('@/api/dashboard', () => ({ getDashboardSummary: dashboardMocks.getDashboardSummary }));
vi.mock('@/api/employees', () => ({ getEmployees: employeeMocks.getEmployees }));

import { DashboardPage } from '@/pages/dashboard/DashboardPage';

function asUser(roles: AppRole[], overrides: Partial<User> = {}): User {
  return userWithRoles(roles, { full_name: 'Nurul Izzah', ...overrides });
}

const sampleSummary: DashboardSummary = {
  total_employees: 42,
  active_employees: 38,
  last_payroll_period: '2026-10',
  last_payroll_total_net: 18_540_000,
  last_payroll_total_gross: 21_200_000,
  last_payroll_employee_count: 38,
  ytd_total_gross: 210_000_000,
  ytd_total_epf_employer: 25_200_000,
  ytd_total_socso_employer: 3_100_000,
  ytd_total_eis_employer: 420_000,
  departments: [
    { department: 'Engineering', count: 20 },
    { department: 'Unassigned', count: 5 },
  ],
  needs_attention: {
    window_days: 7,
    late_count: 3,
    absent_count: 1,
    open_session_count: 2,
    outside_geofence_count: 0,
    manual_entry_count: 0,
    pending_leave: 4,
    pending_claims: 2,
    pending_overtime: 0,
    all_clear: false,
  },
};

beforeEach(() => {
  authMocks.useAuth.mockReset();
  dashboardMocks.getDashboardSummary.mockReset();
  employeeMocks.getEmployees.mockReset().mockResolvedValue({ data: [], total: 0 });
  document.documentElement.classList.remove('dark');
});

describe('getErrorMessage 429 rate-limit handling', () => {
  it('surfaces rateLimitMessage attached by the Axios interceptor', () => {
    expect(
      getErrorMessage({
        rateLimitMessage: 'Too many requests. Try again in 12 seconds.',
        response: { status: 429 },
      }),
    ).toBe('Too many requests. Try again in 12 seconds.');
  });

  it('localizes bare 429 responses when retry-after header is present', () => {
    expect(
      getErrorMessage({
        response: { status: 429, headers: { 'retry-after': '15' } },
      }),
    ).toContain('15');
  });
});

describe('ToastProvider & imperative toast', () => {
  function Trigger() {
    const t = useToast();
    return (
      <div>
        <button type="button" onClick={() => t.success('Employee saved')}>
          Save
        </button>
        <button type="button" onClick={() => toast.error('Mutation failed')}>
          Fail
        </button>
      </div>
    );
  }

  it('renders hook and imperative toasts with accessible roles and allows dismissal', async () => {
    const user = userEvent.setup();
    render(
      <ToastProvider>
        <Trigger />
      </ToastProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByRole('status')).toHaveTextContent('Employee saved');

    await user.click(screen.getByRole('button', { name: 'Fail' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Mutation failed');

    const dismissButtons = screen.getAllByRole('button', { name: /dismiss notification/i });
    await user.click(dismissButtons[0]);
    await waitFor(() => expect(screen.queryByText('Employee saved')).not.toBeInTheDocument());
  });
});

describe('ConfirmProvider & useConfirm', () => {
  function DeleteHarness() {
    const confirm = useConfirm();
    const [outcome, setOutcome] = useState('idle');
    return (
      <div>
        <button
          type="button"
          onClick={async () => {
            const ok = await confirm({
              description: 'Delete October payroll draft?',
              variant: 'danger',
            });
            setOutcome(ok ? 'confirmed' : 'cancelled');
          }}
        >
          Trigger Delete
        </button>
        <span data-testid="outcome">{outcome}</span>
      </div>
    );
  }

  it('resolves true on confirm and false on Escape/cancel', async () => {
    const user = userEvent.setup();
    render(
      <ConfirmProvider>
        <DeleteHarness />
      </ConfirmProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Trigger Delete' }));
    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveTextContent('Delete October payroll draft?');

    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.getByTestId('outcome')).toHaveTextContent('cancelled'));

    await user.click(screen.getByRole('button', { name: 'Trigger Delete' }));
    const dialog2 = await screen.findByRole('dialog');
    await user.click(within(dialog2).getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(screen.getByTestId('outcome')).toHaveTextContent('confirmed'));
  });
});

describe('DataTable keyboard accessibility and sorting', () => {
  interface Item {
    id: string;
    name: string;
    score: number;
  }

  const cols: Column<Item>[] = [
    { key: 'name', header: 'Name', render: (r) => r.name, sortValue: (r) => r.name, primary: true },
    { key: 'score', header: 'Score', align: 'right', render: (r) => r.score, sortValue: (r) => r.score },
  ];

  const rows: Item[] = [
    { id: '1', name: 'Zara', score: 20 },
    { id: '2', name: 'Amir', score: 90 },
    { id: '3', name: 'Mei', score: 50 },
  ];

  it('opens row summary via Enter key on focused row and sorts client-side columns', async () => {
    const user = userEvent.setup();
    render(<DataTable columns={cols} data={rows} summaryTitle={(r) => `Details: ${r.name}`} />);

    const tbl = screen.getByRole('table');
    await user.click(within(tbl).getByRole('button', { name: /Sort by Name/i }));

    const bodyRows = within(tbl.querySelector('tbody') as HTMLElement).getAllByRole('row');
    expect(bodyRows[0]).toHaveTextContent('Amir');
    expect(bodyRows[2]).toHaveTextContent('Zara');

    bodyRows[0].focus();
    await user.keyboard('{Enter}');
    expect(await screen.findByRole('dialog')).toHaveTextContent('Details: Amir');
  });
});

describe('DashboardPage', () => {
  function renderDashboard(roles: AppRole[]) {
    authMocks.useAuth.mockReturnValue({
      user: asUser(roles),
      isAuthenticated: true,
      isLoading: false,
    });
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    return render(
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/dashboard']}>
          <Routes>
            <Route path="/dashboard" element={<DashboardPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );
  }

  it('shows payroll & statutory cards to payroll_admin and hides them from exec', async () => {
    dashboardMocks.getDashboardSummary.mockResolvedValue(sampleSummary);

    const { unmount } = renderDashboard(['payroll_admin']);
    expect(await screen.findByText('Latest Net Payroll')).toBeInTheDocument();
    expect(screen.getByText('Employer Statutory Breakdown (YTD)')).toBeInTheDocument();
    expect(screen.getByText('Pending Leave')).toBeInTheDocument();
    expect(screen.getByText('Unassigned')).toBeInTheDocument();
    unmount();

    renderDashboard(['exec']);
    expect(await screen.findByText('Active Employees')).toBeInTheDocument();
    expect(screen.queryByText('Latest Net Payroll')).not.toBeInTheDocument();
    expect(screen.queryByText('Employer Statutory Breakdown (YTD)')).not.toBeInTheDocument();
  });

  it('renders the All Clear banner when needs_attention.all_clear is true', async () => {
    dashboardMocks.getDashboardSummary.mockResolvedValue({
      ...sampleSummary,
      needs_attention: {
        window_days: 7,
        late_count: 0,
        absent_count: 0,
        open_session_count: 0,
        outside_geofence_count: 0,
        manual_entry_count: 0,
        pending_leave: 0,
        pending_claims: 0,
        pending_overtime: 0,
        all_clear: true,
      },
    });

    renderDashboard(['hr_manager']);
    expect(await screen.findByText('Everything is caught up')).toBeInTheDocument();
  });
});

describe('ThemeProvider & CommandPalette', () => {
  it('defaults to system appearance and follows prefers-color-scheme when unset', () => {
    localStorage.removeItem('payroll-appearance');
    document.documentElement.classList.remove('dark');
    const origMatchMedia = window.matchMedia;
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: query === '(prefers-color-scheme: dark)',
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));

    try {
      render(
        <ThemeProvider>
          <AppearanceSwitcher />
        </ThemeProvider>,
      );

      expect(screen.getByRole('button', { name: 'System' })).toHaveAttribute('aria-pressed', 'true');
      expect(document.documentElement.classList.contains('dark')).toBe(true);
    } finally {
      window.matchMedia = origMatchMedia;
      localStorage.removeItem('payroll-appearance');
      document.documentElement.classList.remove('dark');
    }
  });

  it('toggles dark class on documentElement via AppearanceSwitcher', async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <AppearanceSwitcher />
      </ThemeProvider>,
    );

    expect(document.documentElement.classList.contains('dark')).toBe(false);
    await user.click(screen.getByRole('button', { name: 'Dark' }));
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    await user.click(screen.getByRole('button', { name: 'Light' }));
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    await user.click(screen.getByRole('button', { name: 'System' }));
    expect(localStorage.getItem('payroll-appearance')).toBe('system');
  });

  it('opens CommandPalette, filters routes by permission, and searches employees live', async () => {
    const user = userEvent.setup();
    authMocks.useAuth.mockReturnValue({
      user: asUser(['hr_manager']),
      isAuthenticated: true,
      isLoading: false,
    });
    employeeMocks.getEmployees.mockResolvedValue({
      data: [{ id: 'emp-99', full_name: 'Sarah Ahmad', employee_number: 'EMP-099', department: 'Engineering' }],
      total: 1,
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <ThemeProvider>
          <MemoryRouter initialEntries={['/dashboard']}>
            <CommandPalette />
          </MemoryRouter>
        </ThemeProvider>
      </QueryClientProvider>,
    );

    act(() => {
      openCommandPalette();
    });
    const dialog = await screen.findByRole('dialog', { name: /Quick navigation/i });
    expect(within(dialog).getByRole('button', { name: 'Employees' })).toBeInTheDocument();
    // hr_manager does not hold view_payroll or manage_backups
    expect(within(dialog).queryByRole('button', { name: 'Payroll' })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole('button', { name: 'Backup' })).not.toBeInTheDocument();

    const searchInput = within(dialog).getByRole('textbox');
    await user.type(searchInput, 'Sarah');
    expect(await within(dialog).findByText('Sarah Ahmad')).toBeInTheDocument();
    expect(within(dialog).getByText(/EMP-099/)).toBeInTheDocument();
  });
});

describe('PromptProvider & usePrompt', () => {
  function CancelPayrollHarness() {
    const prompt = usePrompt();
    const [result, setResult] = useState('idle');
    return (
      <div>
        <button
          type="button"
          onClick={async () => {
            const reason = await prompt({
              title: 'Cancel Payroll Run',
              description: 'Why is this payroll run being cancelled?',
              required: true,
              variant: 'danger',
            });
            setResult(reason === null ? 'dismissed' : `reason:${reason}`);
          }}
        >
          Cancel Run
        </button>
        <span data-testid="prompt-result">{result}</span>
      </div>
    );
  }

  it('enforces required text and resolves trimmed reason', async () => {
    const user = userEvent.setup();
    render(
      <PromptProvider>
        <CancelPayrollHarness />
      </PromptProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Cancel Run' }));
    const dialog = await screen.findByRole('dialog', { name: 'Cancel Payroll Run' });

    // Submitting blank when required=true keeps dialog open with alert
    await user.click(within(dialog).getByRole('button', { name: 'Confirm' }));
    expect(await within(dialog).findByRole('alert')).toBeInTheDocument();

    await user.type(within(dialog).getByRole('textbox'), '  Duplicate OT entry  ');
    await user.click(within(dialog).getByRole('button', { name: 'Confirm' }));
    await waitFor(() =>
      expect(screen.getByTestId('prompt-result')).toHaveTextContent('reason:Duplicate OT entry'),
    );
  });
});

describe('Nested Modal stack, Escape isolation & Dashboard unassigned guard', () => {
  it('routes Escape exclusively to the topmost Modal and ref-counts body scroll lock', async () => {
    const user = userEvent.setup();
    const onParentClose = vi.fn();
    const onChildClose = vi.fn();

    function NestedHarness() {
      const [parentOpen, setParentOpen] = useState(true);
      const [childOpen, setChildOpen] = useState(true);
      return (
        <>
          <Modal
            open={parentOpen}
            onClose={() => {
              onParentClose();
              setParentOpen(false);
            }}
            title="Parent Modal"
          >
            <p>Parent body</p>
          </Modal>
          <Modal
            open={childOpen}
            onClose={() => {
              onChildClose();
              setChildOpen(false);
            }}
            title="Child Modal"
          >
            <p>Child body</p>
          </Modal>
        </>
      );
    }

    render(<NestedHarness />);
    expect(await screen.findByRole('dialog', { name: 'Parent Modal' })).toBeInTheDocument();
    expect(await screen.findByRole('dialog', { name: 'Child Modal' })).toBeInTheDocument();
    expect(document.body.style.overflow).toBe('hidden');

    // First Escape closes only Child Modal; Parent remains open and scroll stays locked
    await user.keyboard('{Escape}');
    expect(onChildClose).toHaveBeenCalledTimes(1);
    expect(onParentClose).not.toHaveBeenCalled();
    expect(document.body.style.overflow).toBe('hidden');

    // Second Escape closes Parent Modal and releases body scroll lock
    await user.keyboard('{Escape}');
    expect(onParentClose).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(document.body.style.overflow).toBe(''));
  });

  it('closes CommandPalette on Escape without closing an underlying open Modal', async () => {
    const user = userEvent.setup();
    const onModalClose = vi.fn();
    authMocks.useAuth.mockReturnValue({
      user: asUser(['admin']),
      isAuthenticated: true,
      isLoading: false,
    });

    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <ThemeProvider>
          <MemoryRouter initialEntries={['/dashboard']}>
            <Modal open onClose={onModalClose} title="Underlying Modal">
              <p>Keep me open</p>
            </Modal>
            <CommandPalette />
          </MemoryRouter>
        </ThemeProvider>
      </QueryClientProvider>,
    );

    act(() => {
      openCommandPalette();
    });
    expect(await screen.findByRole('dialog', { name: /Quick navigation/i })).toBeInTheDocument();

    await user.keyboard('{Escape}');
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: /Quick navigation/i })).not.toBeInTheDocument(),
    );
    expect(onModalClose).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog', { name: 'Underlying Modal' })).toBeInTheDocument();
  });

  it('redirects unassigned super_admin away from /dashboard to /companies without calling dashboard API', async () => {
    authMocks.useAuth.mockReturnValue({
      user: asUser(['super_admin'], { company_id: null }),
      isAuthenticated: true,
      isLoading: false,
    });
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/dashboard']}>
          <Routes>
            <Route path="/dashboard" element={<DashboardPage />} />
            <Route path="/companies" element={<div>Companies Directory</div>} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(await screen.findByText('Companies Directory')).toBeInTheDocument();
    expect(dashboardMocks.getDashboardSummary).not.toHaveBeenCalled();
  });
});
