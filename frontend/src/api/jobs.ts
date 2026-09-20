import api from './client';
import type { BackgroundJob } from '@/types';

/// Poll a submitted background job. Pair with `refetchInterval` until
/// `status` leaves `pending`/`running`; `result` then carries the job's
/// outcome (a `{run_id}` for payroll, the confirm body for imports).
export async function getJob(id: string): Promise<BackgroundJob> {
  const { data } = await api.get(`/jobs/${id}`);
  return data;
}
