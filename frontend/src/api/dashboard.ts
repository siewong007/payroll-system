import api from './client';
import type { DashboardSummary } from '@/types';

export async function getDashboardSummary(): Promise<DashboardSummary> {
  const { data } = await api.get('/dashboard/summary');
  return data;
}
