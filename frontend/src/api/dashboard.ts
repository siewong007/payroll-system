import api from './client';
import type { DashboardSummary } from '@/types';

export async function getDashboardSummary(windowDays = 7): Promise<DashboardSummary> {
  const { data } = await api.get<DashboardSummary>('/dashboard/summary', {
    params: { window_days: windowDays },
  });
  return data;
}
