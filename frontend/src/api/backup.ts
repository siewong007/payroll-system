import api from './client';
import type { ImportResult } from '@/types';

export async function exportCompanyBackup(): Promise<void> {
  const response = await api.get('/admin/backup/export', { responseType: 'blob' });
  const url = window.URL.createObjectURL(new Blob([response.data]));
  const a = document.createElement('a');
  a.href = url;
  const disposition = response.headers['content-disposition'];
  a.download = disposition?.split('filename=')[1]?.replace(/"/g, '') || 'backup.json';
  document.body.appendChild(a);
  a.click();
  a.remove();
  window.URL.revokeObjectURL(url);
}

export async function importCompanyBackup(file: File): Promise<ImportResult> {
  const formData = new FormData();
  formData.append('file', file);
  const { data } = await api.post('/admin/backup/import', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
    timeout: 300000, // 5 min for large imports
  });
  return data;
}
