import api from './client';
import type {
  AuditLog,
  BackgroundJob,
  CreatePayrollEntryRequest,
  JournalPreview,
  PayrollOverview,
  PayrollEntry,
  PayrollEntryWithEmployee,
  PayrollGroup,
  PayrollPreview,
  PayrollRun,
  PayrollSummary,
  PayslipBreakdown,
  ProcessPayrollRequest,
  UpdatePayrollEntryRequest,
  UpdatePayrollPcbRequest,
} from '@/types';

export async function getPayrollGroups(): Promise<PayrollGroup[]> {
  const { data } = await api.get('/payroll-groups');
  return data;
}

export async function getPayrollRuns(): Promise<PayrollRun[]> {
  const { data } = await api.get('/payroll/runs');
  return data;
}

export async function getPayrollRun(id: string): Promise<PayrollSummary> {
  const { data } = await api.get(`/payroll/runs/${id}`);
  return data;
}

export async function getPayrollRunAuditLogs(id: string): Promise<AuditLog[]> {
  const { data } = await api.get(`/payroll/runs/${id}/audit-logs`);
  return data;
}

export async function deletePayrollRun(id: string): Promise<void> {
  await api.delete(`/payroll/runs/${id}`);
}

/// Submits the run for background processing — the response is the job, not
/// the run. Poll `getJob` until it leaves `pending`/`running`; `result.run_id`
/// then names the committed run.
export async function processPayroll(req: ProcessPayrollRequest): Promise<BackgroundJob> {
  const { data } = await api.post('/payroll/run', req);
  return data;
}

/// Dry-run the same calculation `processPayroll` would commit. Writes nothing.
export async function previewPayroll(req: ProcessPayrollRequest): Promise<PayrollPreview> {
  const { data } = await api.post('/payroll/preview', req);
  return data;
}

export async function getPayslipBreakdown(
  runId: string,
  employeeId: string,
): Promise<PayslipBreakdown> {
  const { data } = await api.get(`/payroll/runs/${runId}/items/${employeeId}/breakdown`);
  return data;
}

export async function getPayrollEntries(params?: {
  period_year?: number;
  period_month?: number;
  employee_id?: string;
  item_type?: string;
  include_processed?: boolean;
}): Promise<PayrollEntryWithEmployee[]> {
  const { data } = await api.get('/payroll/entries', { params });
  return data;
}

export async function createPayrollEntry(req: CreatePayrollEntryRequest): Promise<PayrollEntry> {
  const { data } = await api.post('/payroll/entries', req);
  return data;
}

export async function updatePayrollEntry(id: string, req: UpdatePayrollEntryRequest): Promise<PayrollEntry> {
  const { data } = await api.put(`/payroll/entries/${id}`, req);
  return data;
}

export async function deletePayrollEntry(id: string): Promise<void> {
  await api.delete(`/payroll/entries/${id}`);
}

export async function updatePayrollItemPcb(
  runId: string,
  employeeId: string,
  req: UpdatePayrollPcbRequest,
): Promise<PayrollSummary> {
  const { data } = await api.put(`/payroll/runs/${runId}/items/${employeeId}/pcb`, req);
  return data;
}

export async function submitPayrollForApproval(id: string): Promise<PayrollRun> {
  const { data } = await api.put(`/payroll/runs/${id}/submit-approval`);
  return data;
}

export async function approvePayroll(id: string): Promise<PayrollRun> {
  const { data } = await api.put(`/payroll/runs/${id}/approve`);
  return data;
}

export async function returnPayrollForChanges(id: string, reason?: string): Promise<PayrollRun> {
  const { data } = await api.put(`/payroll/runs/${id}/return`, { reason });
  return data;
}

export async function lockPayroll(id: string): Promise<PayrollRun> {
  const { data } = await api.put(`/payroll/runs/${id}/lock`);
  return data;
}

export async function downloadRunPayslips(runId: string): Promise<void> {
  const res = await api.get(`/payroll/runs/${runId}/payslips/pdf`, { responseType: 'blob' });
  const url = window.URL.createObjectURL(new Blob([res.data]));
  const a = document.createElement('a');
  a.href = url;
  a.download = `payslips_${runId}.pdf`;
  a.click();
  window.URL.revokeObjectURL(url);
}

/// Operational dashboard payload: committed period totals, pipeline, variance,
/// department split and the live action queue.
export async function getPayrollOverview(): Promise<PayrollOverview> {
  const { data } = await api.get('/payroll/overview');
  return data;
}

/// Cancel a run before money moves. `reason` is mandatory and is written to
/// the run row and the audit trail. `paid` runs cannot be cancelled.
export async function cancelPayrollRun(id: string, reason: string): Promise<PayrollRun> {
  const { data } = await api.put(`/payroll/runs/${id}/cancel`, { reason });
  return data;
}

/// Bank payment file (CSV) for an approved/paid run — finance-facing.
export async function downloadPaymentFile(runId: string): Promise<void> {
  const res = await api.get(`/payroll/runs/${runId}/payment-file`, { responseType: 'blob' });
  const url = window.URL.createObjectURL(new Blob([res.data]));
  const a = document.createElement('a');
  a.href = url;
  a.download = `payment_file_${runId}.csv`;
  a.click();
  window.URL.revokeObjectURL(url);
}

/// Balanced journal preview for an approved/paid run. Writes nothing.
export async function getJournalPreview(runId: string): Promise<JournalPreview> {
  const { data } = await api.get(`/payroll/runs/${runId}/journal-preview`);
  return data;
}
