import { beforeEach, describe, expect, it, vi } from 'vitest';

const apiMocks = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
  put: vi.fn(),
  delete: vi.fn(),
}));

vi.mock('@/api/client', () => ({ default: apiMocks }));

import {
  approvePayroll,
  deletePayrollRun,
  downloadRunPayslips,
  getPayrollEntries,
  getPayrollRun,
  getPayrollRunAuditLogs,
  getPayslipBreakdown,
  lockPayroll,
  previewPayroll,
  processPayroll,
  returnPayrollForChanges,
  submitPayrollForApproval,
  updatePayrollItemPcb,
} from '@/api/payroll';
import {
  downloadEaForm,
  downloadStatutoryExport,
  getClaimsReport,
  getPayrollByDepartment,
  getPayrollSummary,
  getStatutoryReport,
} from '@/api/reports';
import { getEmployee, getEmployees, updateEmployee } from '@/api/employees';
import { createKioskCredential, fetchKioskQr, revokeKioskCredential } from '@/api/kiosk';

/** Captures the synthetic <a download> the browser-download helpers create. */
function captureDownload() {
  const realCreateElement = document.createElement.bind(document);
  const anchor = realCreateElement('a');
  const click = vi.fn();
  anchor.click = click;
  vi.spyOn(document, 'createElement').mockImplementation((tag: string) =>
    tag === 'a' ? anchor : realCreateElement(tag as 'div'),
  );
  return { anchor, click };
}

beforeEach(() => {
  apiMocks.get.mockReset().mockResolvedValue({ data: {}, headers: {} });
  apiMocks.post.mockReset().mockResolvedValue({ data: {} });
  apiMocks.put.mockReset().mockResolvedValue({ data: {} });
  apiMocks.delete.mockReset().mockResolvedValue({ data: {} });
  vi.restoreAllMocks();

  globalThis.URL.createObjectURL = vi.fn(() => 'blob:mock-url');
  globalThis.URL.revokeObjectURL = vi.fn();
});

describe('payroll API', () => {
  it('unwraps the response body rather than returning the axios envelope', async () => {
    apiMocks.get.mockResolvedValue({ data: { id: 'run-1', total_net: '12345.67' } });

    await expect(getPayrollRun('run-1')).resolves.toEqual({ id: 'run-1', total_net: '12345.67' });
    expect(apiMocks.get).toHaveBeenCalledWith('/payroll/runs/run-1');
  });

  it('forwards entry filters as query params', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await getPayrollEntries({ period_year: 2026, period_month: 7, include_processed: false });

    expect(apiMocks.get).toHaveBeenCalledWith('/payroll/entries', {
      params: { period_year: 2026, period_month: 7, include_processed: false },
    });
  });

  it('omits the params object entirely when no filters are supplied', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await getPayrollEntries();

    expect(apiMocks.get).toHaveBeenCalledWith('/payroll/entries', { params: undefined });
  });

  it('posts a process request to the run endpoint', async () => {
    apiMocks.post.mockResolvedValue({ data: { id: 'run-9' } });

    await processPayroll({
      payroll_group_id: 'group-1',
      period_year: 2026,
      period_month: 7,
    } as Parameters<typeof processPayroll>[0]);

    expect(apiMocks.post).toHaveBeenCalledWith('/payroll/run', {
      payroll_group_id: 'group-1',
      period_year: 2026,
      period_month: 7,
    });
  });

  it('previews against its own endpoint so no run is created', async () => {
    apiMocks.post.mockResolvedValue({ data: { can_process: true } });

    const request = {
      payroll_group_id: 'group-1',
      period_year: 2026,
      period_month: 7,
    } as Parameters<typeof previewPayroll>[0];

    await expect(previewPayroll(request)).resolves.toEqual({ can_process: true });

    // The preview must never reach /payroll/run — that endpoint commits.
    expect(apiMocks.post).toHaveBeenCalledWith('/payroll/preview', request);
    expect(apiMocks.post).not.toHaveBeenCalledWith('/payroll/run', expect.anything());
  });

  it('fetches a payslip breakdown scoped to both the run and the employee', async () => {
    apiMocks.get.mockResolvedValue({ data: { lines: [] } });

    await getPayslipBreakdown('run-1', 'emp-7');

    expect(apiMocks.get).toHaveBeenCalledWith('/payroll/runs/run-1/items/emp-7/breakdown');
  });

  it('drives each lifecycle transition through its own endpoint', async () => {
    await submitPayrollForApproval('run-1');
    expect(apiMocks.put).toHaveBeenLastCalledWith('/payroll/runs/run-1/submit-approval');

    await approvePayroll('run-1');
    expect(apiMocks.put).toHaveBeenLastCalledWith('/payroll/runs/run-1/approve');

    await lockPayroll('run-1');
    expect(apiMocks.put).toHaveBeenLastCalledWith('/payroll/runs/run-1/lock');

    await deletePayrollRun('run-1');
    expect(apiMocks.delete).toHaveBeenLastCalledWith('/payroll/runs/run-1');
  });

  it('carries the reviewer reason when returning a run for changes', async () => {
    await returnPayrollForChanges('run-1', 'Overtime hours look wrong');

    expect(apiMocks.put).toHaveBeenCalledWith('/payroll/runs/run-1/return', {
      reason: 'Overtime hours look wrong',
    });
  });

  it('still sends a body when the return reason is omitted', async () => {
    await returnPayrollForChanges('run-1');

    expect(apiMocks.put).toHaveBeenCalledWith('/payroll/runs/run-1/return', { reason: undefined });
  });

  it('scopes a manual PCB override to one employee within one run', async () => {
    // PCB is entered manually until the calculator passes LHDN conformance, so
    // the override must never be addressable at run level alone.
    await updatePayrollItemPcb('run-1', 'emp-7', { pcb_amount: 25000 } as Parameters<
      typeof updatePayrollItemPcb
    >[2]);

    expect(apiMocks.put).toHaveBeenCalledWith('/payroll/runs/run-1/items/emp-7/pcb', {
      pcb_amount: 25000,
    });
  });

  it('reads a run audit trail from the run-scoped endpoint', async () => {
    apiMocks.get.mockResolvedValue({ data: [{ id: 'log-1' }] });

    await expect(getPayrollRunAuditLogs('run-1')).resolves.toEqual([{ id: 'log-1' }]);
    expect(apiMocks.get).toHaveBeenCalledWith('/payroll/runs/run-1/audit-logs');
  });

  it('requests payslips as a blob and names the download after the run', async () => {
    const { anchor, click } = captureDownload();
    apiMocks.get.mockResolvedValue({ data: new Blob(['pdf']), headers: {} });

    await downloadRunPayslips('run-42');

    expect(apiMocks.get).toHaveBeenCalledWith('/payroll/runs/run-42/payslips/pdf', {
      responseType: 'blob',
    });
    expect(anchor.download).toBe('payslips_run-42.pdf');
    expect(click).toHaveBeenCalledOnce();
    expect(globalThis.URL.revokeObjectURL).toHaveBeenCalledWith('blob:mock-url');
  });
});

describe('reports API', () => {
  it('sends period selectors as typed query params', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await getPayrollSummary(2026);
    expect(apiMocks.get).toHaveBeenLastCalledWith('/reports/payroll-summary', {
      params: { year: 2026 },
    });

    await getPayrollByDepartment(2026, 7);
    expect(apiMocks.get).toHaveBeenLastCalledWith('/reports/payroll-department', {
      params: { year: 2026, month: 7 },
    });

    await getStatutoryReport(2026, 7);
    expect(apiMocks.get).toHaveBeenLastCalledWith('/reports/statutory', {
      params: { year: 2026, month: 7 },
    });
  });

  it('maps camelCase arguments onto the snake_case contract', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await getClaimsReport('2026-01-01', '2026-06-30');

    expect(apiMocks.get).toHaveBeenCalledWith('/reports/claims', {
      params: { start_date: '2026-01-01', end_date: '2026-06-30' },
    });
  });

  it('honours the server-supplied filename for a statutory export', async () => {
    const { anchor } = captureDownload();
    apiMocks.get.mockResolvedValue({
      data: new Blob(['csv']),
      headers: { 'content-disposition': 'attachment; filename="EPF_202607.csv"' },
    });

    await downloadStatutoryExport('epf', 2026, 7);

    expect(apiMocks.get).toHaveBeenCalledWith('/reports/statutory/epf-export', {
      params: { year: 2026, month: 7 },
      responseType: 'blob',
    });
    expect(anchor.download).toBe('EPF_202607.csv');
  });

  it('falls back to a derived filename when the header is absent', async () => {
    const { anchor } = captureDownload();
    apiMocks.get.mockResolvedValue({ data: new Blob(['csv']), headers: {} });

    await downloadStatutoryExport('socso', 2026, 3);

    expect(anchor.download).toBe('socso_export_2026_3.csv');
  });

  it('parses an unquoted filename', async () => {
    const { anchor } = captureDownload();
    apiMocks.get.mockResolvedValue({
      data: new Blob(['csv']),
      headers: { 'content-disposition': 'attachment; filename=EIS_202607.csv' },
    });

    await downloadStatutoryExport('eis', 2026, 7);

    expect(anchor.download).toBe('EIS_202607.csv');
  });

  it('downloads an EA form scoped to one employee and tax year', async () => {
    const { anchor } = captureDownload();
    apiMocks.get.mockResolvedValue({ data: new Blob(['pdf']), headers: {} });

    await downloadEaForm(2026, 'emp-3');

    expect(apiMocks.get).toHaveBeenCalledWith('/reports/ea-form', {
      params: { year: 2026, employee_id: 'emp-3' },
      responseType: 'blob',
    });
    expect(anchor.download).toBe('EA_Form_2026.pdf');
  });
});

describe('employees API', () => {
  it('passes list filters and pagination through', async () => {
    apiMocks.get.mockResolvedValue({ data: { data: [], total: 0, page: 1, per_page: 20 } });

    await getEmployees({ search: 'siti', department: 'Finance', is_active: true, page: 2, per_page: 20 });

    expect(apiMocks.get).toHaveBeenCalledWith('/employees', {
      params: { search: 'siti', department: 'Finance', is_active: true, page: 2, per_page: 20 },
    });
  });

  it('preserves is_active=false rather than dropping a falsy filter', async () => {
    apiMocks.get.mockResolvedValue({ data: { data: [], total: 0, page: 1, per_page: 20 } });

    await getEmployees({ is_active: false });

    expect(apiMocks.get).toHaveBeenCalledWith('/employees', { params: { is_active: false } });
  });

  it('reads and partially updates a single employee', async () => {
    apiMocks.get.mockResolvedValue({ data: { id: 'emp-1' } });
    await expect(getEmployee('emp-1')).resolves.toEqual({ id: 'emp-1' });

    await updateEmployee('emp-1', { department: 'Engineering' });
    expect(apiMocks.put).toHaveBeenCalledWith('/employees/emp-1', { department: 'Engineering' });
  });

  it('sends the explicit resignation clear rather than an absent date', async () => {
    apiMocks.put.mockResolvedValue({ data: { id: 'emp-1' } });

    // Omitting date_resigned means "keep existing" on the backend, so an
    // un-termination has to travel as its own flag or it is silently a no-op.
    await updateEmployee('emp-1', { clear_date_resigned: true });

    expect(apiMocks.put).toHaveBeenCalledWith('/employees/emp-1', { clear_date_resigned: true });
  });

  it('sends a recorded resignation date and reason', async () => {
    apiMocks.put.mockResolvedValue({ data: { id: 'emp-1' } });

    await updateEmployee('emp-1', { date_resigned: '2026-05-15', resignation_reason: 'New role' });

    expect(apiMocks.put).toHaveBeenCalledWith('/employees/emp-1', {
      date_resigned: '2026-05-15',
      resignation_reason: 'New role',
    });
  });
});

describe('kiosk API', () => {
  it('authenticates the public QR fetch with the kiosk secret, not the user JWT', async () => {
    apiMocks.post.mockResolvedValue({ data: { token: 'qr-token', ttl_seconds: 300 } });

    const result = await fetchKioskQr('kiosk-secret');

    // An explicit Authorization header makes the client interceptor leave the
    // user's Bearer token off this request entirely.
    expect(apiMocks.post).toHaveBeenCalledWith('/attendance/kiosk/qr', null, {
      headers: { Authorization: 'Kiosk kiosk-secret' },
    });
    expect(result.ttl_seconds).toBe(300);
  });

  it('creates and revokes kiosk credentials', async () => {
    apiMocks.post.mockResolvedValue({ data: { secret: 'one-time-secret' } });

    await expect(createKioskCredential('Lobby tablet')).resolves.toEqual({
      secret: 'one-time-secret',
    });
    expect(apiMocks.post).toHaveBeenCalledWith('/attendance/kiosks', { label: 'Lobby tablet' });

    await expect(revokeKioskCredential('cred-1')).resolves.toBeUndefined();
    expect(apiMocks.delete).toHaveBeenCalledWith('/attendance/kiosks/cred-1');
  });
});
