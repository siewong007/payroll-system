import api from './client';
import { triggerBlobDownload } from '@/lib/download';
import type { ImportResult, StepUpCredentials } from '@/types';

export async function exportCompanyBackup(
  companyId: string | undefined,
  stepUp: StepUpCredentials,
): Promise<void> {
  const body = {
    ...(companyId ? { company_id: companyId } : {}),
    ...stepUp,
  };
  const response = await api.post('/admin/backup/export', body, { responseType: 'blob' });
  const disposition = response.headers['content-disposition'];
  const filename = disposition?.split('filename=')[1]?.replace(/"/g, '') || 'backup.json';
  triggerBlobDownload(response.data, filename);
}

export async function importCompanyBackup(
  file: File,
  companyId: string | undefined,
  createNew: boolean,
  stepUp: StepUpCredentials,
): Promise<ImportResult> {
  const formData = new FormData();
  formData.append('file', file);
  if (companyId) formData.append('company_id', companyId);
  if (createNew) formData.append('create_new', 'true');
  formData.append('actor_password', stepUp.actor_password);
  if (stepUp.actor_totp_code) formData.append('actor_totp_code', stepUp.actor_totp_code);
  const { data } = await api.post('/admin/backup/import', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
    timeout: 300000, // 5 min for large imports
  });
  return data;
}
