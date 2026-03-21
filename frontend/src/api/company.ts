import api from './client';
import type { Company, UpdateCompanyRequest, CompanyStats } from '@/types';

export async function getCompany(): Promise<Company> {
  const { data } = await api.get('/company');
  return data;
}

export async function updateCompany(req: UpdateCompanyRequest): Promise<Company> {
  const { data } = await api.put('/company', req);
  return data;
}

export async function getCompanyStats(): Promise<CompanyStats> {
  const { data } = await api.get('/company/stats');
  return data;
}
