import api from './client';
import type { Claim } from '@/types';

export async function listMyClaims(status?: string): Promise<Claim[]> {
  const { data } = await api.get('/portal/claims', { params: { status } });
  return data;
}

export async function createClaim(req: {
  title: string;
  description?: string;
  amount: number;
  category?: string;
  receipt_url?: string;
  receipt_file_name?: string;
  expense_date: string;
}): Promise<Claim> {
  const { data } = await api.post('/portal/claims', req);
  return data;
}

export async function submitClaim(id: string): Promise<Claim> {
  const { data } = await api.put(`/portal/claims/${id}/submit`);
  return data;
}

export async function cancelClaim(id: string): Promise<void> {
  await api.put(`/portal/claims/${id}/cancel`);
}

export async function deleteClaim(id: string): Promise<void> {
  await api.delete(`/portal/claims/${id}`);
}

export async function uploadFile(file: File): Promise<{ url: string; file_name: string; size: number }> {
  const formData = new FormData();
  formData.append('file', file);
  const { data } = await api.post('/uploads', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
  });
  return data;
}
