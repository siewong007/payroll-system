import api from './client';

export interface PayrollSummaryRow {
  period: string;
  employee_count: number;
  total_gross: number;
  total_net: number;
  total_epf_employee: number;
  total_epf_employer: number;
  total_socso_employee: number;
  total_socso_employer: number;
  total_eis_employee: number;
  total_eis_employer: number;
  total_pcb: number;
  total_zakat: number;
  total_employer_cost: number;
}

export interface DepartmentPayrollRow {
  department: string | null;
  employee_count: number;
  total_gross: number;
  total_net: number;
  total_employer_cost: number;
}

export interface LeaveReportRow {
  employee_name: string;
  employee_number: string;
  department: string | null;
  leave_type_name: string;
  entitled_days: number;
  taken_days: number;
  pending_days: number;
  balance: number;
}

export interface ClaimsReportRow {
  employee_name: string;
  employee_number: string;
  department: string | null;
  total_claims: number;
  total_amount: number;
  approved_count: number;
  approved_amount: number;
  pending_count: number;
  pending_amount: number;
  rejected_count: number;
}

export interface StatutoryReportRow {
  employee_name: string;
  employee_number: string;
  ic_number: string | null;
  epf_number: string | null;
  socso_number: string | null;
  basic_salary: number;
  gross_salary: number;
  epf_employee: number;
  epf_employer: number;
  socso_employee: number;
  socso_employer: number;
  eis_employee: number;
  eis_employer: number;
  pcb_amount: number;
  zakat_amount: number;
}

export const getPayrollSummary = (year: number) =>
  api.get<PayrollSummaryRow[]>('/reports/payroll-summary', { params: { year } }).then(r => r.data);

export const getPayrollByDepartment = (year: number, month: number) =>
  api.get<DepartmentPayrollRow[]>('/reports/payroll-department', { params: { year, month } }).then(r => r.data);

export const getLeaveReport = (year: number) =>
  api.get<LeaveReportRow[]>('/reports/leave', { params: { year } }).then(r => r.data);

export const getClaimsReport = (startDate: string, endDate: string) =>
  api.get<ClaimsReportRow[]>('/reports/claims', { params: { start_date: startDate, end_date: endDate } }).then(r => r.data);

export const getStatutoryReport = (year: number, month: number) =>
  api.get<StatutoryReportRow[]>('/reports/statutory', { params: { year, month } }).then(r => r.data);
