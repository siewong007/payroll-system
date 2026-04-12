import api from './client';
import type { PayrollGroup, PayrollRun, PayrollSummary, ProcessPayrollRequest } from '@/types';

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

export async function processPayroll(req: ProcessPayrollRequest): Promise<PayrollRun> {
  const { data } = await api.post('/payroll/run', req);
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
