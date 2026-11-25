import api from './client';
import { triggerBlobDownload } from '@/lib/download';
import type { ImportResult } from '@/types';

export async function exportCompanyBackup(companyId?: string): Promise<void> {
  const params = companyId ? { company_id: companyId } : {};
  const response = await api.get('/admin/backup/export', { params, responseType: 'blob' });
  const disposition = response.headers['content-disposition'];
  const filename = disposition?.split('filename=')[1]?.replace(/"/g, '') || 'backup.json';
  triggerBlobDownload(response.data, filename);
}

export async function importCompanyBackup(
  file: File,
  companyId?: string,
  createNew = false,
): Promise<ImportResult> {
  const formData = new FormData();
  formData.append('file', file);
  if (companyId) formData.append('company_id', companyId);
  if (createNew) formData.append('create_new', 'true');
  const { data } = await api.post('/admin/backup/import', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
    timeout: 300000, // 5 min for large imports
  });
  return data;
}
