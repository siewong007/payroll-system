import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ToastProvider } from '@/components/ui/Toast';
import { ConfirmProvider } from '@/components/ui/ConfirmDialog';
import i18n from '@/i18n';
import { userWithRoles } from './support/permissions';

const authMocks = vi.hoisted(() => ({ useAuth: vi.fn() }));
const approvalsMocks = vi.hoisted(() => ({
  getLeaveRequests: vi.fn(),
  getClaims: vi.fn(),
  getOvertimeRequests: vi.fn(),
  approveLeave: vi.fn(),
  rejectLeave: vi.fn(),
  approveClaim: vi.fn(),
  rejectClaim: vi.fn(),
  approveOvertime: vi.fn(),
  rejectOvertime: vi.fn(),
  createLeaveRequest: vi.fn(),
  updateLeaveRequest: vi.fn(),
  cancelLeaveRequest: vi.fn(),
  deleteLeaveRequest: vi.fn(),
  createClaim: vi.fn(),
  updateClaim: vi.fn(),
  cancelClaim: vi.fn(),
  deleteClaim: vi.fn(),
  createOvertimeRequest: vi.fn(),
  updateOvertimeRequest: vi.fn(),
  cancelOvertimeRequest: vi.fn(),
  deleteOvertimeRequest: vi.fn(),
}));
const portalMocks = vi.hoisted(() => ({
  getLeaveTypes: vi.fn(),
  uploadFile: vi.fn(),
}));
const reportsMocks = vi.hoisted(() => ({
  getReportPeriods: vi.fn(),
  getPayrollSummary: vi.fn(),
  getPayrollByDepartment: vi.fn(),
  getLeaveReport: vi.fn(),
  getClaimsReport: vi.fn(),
  getStatutoryReport: vi.fn(),
  downloadStatutoryExport: vi.fn(),
  getEaEmployees: vi.fn(),
  downloadEaForm: vi.fn(),
}));
const employeeMocks = vi.hoisted(() => ({
  getEmployees: vi.fn(),
  createEmployee: vi.fn(),
  updateEmployee: vi.fn(),
  deleteEmployee: vi.fn(),
  getEmployee: vi.fn(),
  getSalaryHistory: vi.fn(),
}));
const payrollMocks = vi.hoisted(() => ({
  getPayrollGroups: vi.fn(),
}));

vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));
vi.mock('@/api/approvals', () => approvalsMocks);
vi.mock('@/api/portal', () => portalMocks);
vi.mock('@/api/reports', () => reportsMocks);
vi.mock('@/api/employees', () => employeeMocks);
vi.mock('@/api/payroll', () => payrollMocks);

import { Approvals } from '@/pages/approvals/Approvals';
import { Reports } from '@/pages/reports/Reports';
import { EmployeeList } from '@/pages/employees/EmployeeList';

function renderWithProviders(ui: React.ReactElement, initialEntries = ['/']) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
        <ToastProvider>
          <ConfirmProvider>{ui}</ConfirmProvider>
        </ToastProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  authMocks.useAuth.mockReturnValue({
    user: userWithRoles(['super_admin'], { full_name: 'Admin User' }),
  });

  approvalsMocks.getLeaveRequests.mockResolvedValue({ data: [], total: 0, page: 1, per_page: 20 });
  approvalsMocks.getClaims.mockResolvedValue({ data: [], total: 0, page: 1, per_page: 20 });
  approvalsMocks.getOvertimeRequests.mockResolvedValue({
    data: [],
    total: 0,
    page: 1,
    per_page: 20,
  });
  portalMocks.getLeaveTypes.mockResolvedValue([]);

  reportsMocks.getReportPeriods.mockResolvedValue({
    default_year: 2026,
    default_month: 11,
    payroll_years: [2026],
    payroll_months: [{ year: 2026, months: [11] }],
    leave_years: [2026],
    claims_years: [2026],
    ea_form_years: [2026],
  });
  reportsMocks.getPayrollSummary.mockResolvedValue([]);
  reportsMocks.getPayrollByDepartment.mockResolvedValue([]);
  reportsMocks.getLeaveReport.mockResolvedValue([]);
  reportsMocks.getClaimsReport.mockResolvedValue([]);
  reportsMocks.getStatutoryReport.mockResolvedValue([]);
  reportsMocks.getEaEmployees.mockResolvedValue([]);

  employeeMocks.getEmployees.mockResolvedValue({
    data: [
      {
        id: 'emp-1',
        employee_number: 'EMP-001',
        full_name: 'Aina Zulkifli',
        department: 'Engineering',
        designation: 'Software Engineer',
        date_joined: '2025-01-15',
        basic_salary: 550000,
        is_active: true,
      },
    ],
    total: 1,
    page: 1,
    per_page: 20,
  });
  payrollMocks.getPayrollGroups.mockResolvedValue([]);
});

describe('Characterization tests before page modularization', () => {
  it('Approvals renders leave tab, switches to claims tab, and opens create claim modal', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Approvals />);

    expect(screen.getByRole('heading', { name: i18n.t('approvals.title') })).toBeInTheDocument();
    await waitFor(() => {
      expect(approvalsMocks.getLeaveRequests).toHaveBeenCalledTimes(1);
    });

    await user.click(screen.getByRole('button', { name: i18n.t('approvals.tabs.claims') }));
    await waitFor(() => {
      expect(approvalsMocks.getClaims).toHaveBeenCalledTimes(1);
    });

    await user.click(screen.getByRole('button', { name: i18n.t('approvals.addClaim') }));
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('Reports renders payroll tab and switches to leave and EA form tabs', async () => {
    const user = userEvent.setup();
    renderWithProviders(<Reports />);

    expect(screen.getByRole('heading', { name: i18n.t('reports.title') })).toBeInTheDocument();
    await waitFor(() => {
      expect(reportsMocks.getPayrollSummary).toHaveBeenCalled();
    });

    await user.click(screen.getByRole('tab', { name: i18n.t('reports.tabs.leave') }));
    await waitFor(() => {
      expect(reportsMocks.getLeaveReport).toHaveBeenCalled();
    });

    await user.click(screen.getByRole('tab', { name: i18n.t('reports.tabs.eaForm') }));
    await waitFor(() => {
      expect(reportsMocks.getEaEmployees).toHaveBeenCalled();
    });
  });

  it('EmployeeList renders employee data and opens create employee modal', async () => {
    const user = userEvent.setup();
    renderWithProviders(<EmployeeList />);

    const matches = await screen.findAllByText('Aina Zulkifli');
    expect(matches.length).toBeGreaterThan(0);

    await user.click(screen.getByRole('button', { name: i18n.t('employees.add') }));
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });
});
