import api from './client';
import type {
  CreatePayrollEntryRequest,
  PayrollEntry,
  PayrollEntryWithEmployee,
  PayrollGroup,
  PayrollRun,
  PayrollSummary,
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

export async function deletePayrollRun(id: string): Promise<void> {
  await api.delete(`/payroll/runs/${id}`);
}

export async function processPayroll(req: ProcessPayrollRequest): Promise<PayrollRun> {
  const { data } = await api.post('/payroll/run', req);
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

export async function approvePayroll(id: string): Promise<PayrollRun> {
  const { data } = await api.put(`/payroll/runs/${id}/approve`);
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
