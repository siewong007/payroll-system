import api from './client';
import type { Employee, PaginatedResponse, CreateEmployeeRequest, SalaryHistory, Tp3Record } from '@/types';

export async function getEmployees(params?: {
  search?: string;
  department?: string;
  is_active?: boolean;
  page?: number;
  per_page?: number;
}): Promise<PaginatedResponse<Employee>> {
  const { data } = await api.get('/employees', { params });
  return data;
}

export async function getEmployee(id: string): Promise<Employee> {
  const { data } = await api.get(`/employees/${id}`);
  return data;
}

export async function createEmployee(req: CreateEmployeeRequest): Promise<Employee> {
  const { data } = await api.post('/employees', req);
  return data;
}

export async function updateEmployee(id: string, req: Partial<CreateEmployeeRequest>): Promise<Employee> {
  const { data } = await api.put(`/employees/${id}`, req);
  return data;
}

export async function deleteEmployee(id: string): Promise<void> {
  await api.delete(`/employees/${id}`);
}

export async function getSalaryHistory(employeeId: string): Promise<SalaryHistory[]> {
  const { data } = await api.get(`/employees/${employeeId}/salary-history`);
  return data;
}

export async function createTp3(employeeId: string, req: {
  tax_year: number;
  previous_employer_name?: string;
  previous_income_ytd: number;
  previous_epf_ytd: number;
  previous_pcb_ytd: number;
  previous_socso_ytd: number;
  previous_zakat_ytd?: number;
}): Promise<Tp3Record> {
  const { data } = await api.post(`/employees/${employeeId}/tp3`, req);
  return data;
}
