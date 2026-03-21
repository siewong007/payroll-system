import api from './client';
import type {
  EmailTemplate,
  CreateEmailTemplateRequest,
  UpdateEmailTemplateRequest,
  EmailLog,
  SendLetterRequest,
  PreviewLetterRequest,
  PreviewLetterResponse,
  PaginatedResponse,
} from '@/types';

// Templates
export async function getEmailTemplates(letterType?: string): Promise<EmailTemplate[]> {
  const { data } = await api.get('/email/templates', { params: { letter_type: letterType } });
  return data;
}

export async function getEmailTemplate(id: string): Promise<EmailTemplate> {
  const { data } = await api.get(`/email/templates/${id}`);
  return data;
}

export async function createEmailTemplate(req: CreateEmailTemplateRequest): Promise<EmailTemplate> {
  const { data } = await api.post('/email/templates', req);
  return data;
}

export async function updateEmailTemplate(id: string, req: UpdateEmailTemplateRequest): Promise<EmailTemplate> {
  const { data } = await api.put(`/email/templates/${id}`, req);
  return data;
}

export async function deleteEmailTemplate(id: string): Promise<void> {
  await api.delete(`/email/templates/${id}`);
}

// Preview & Send
export async function previewLetter(req: PreviewLetterRequest): Promise<PreviewLetterResponse> {
  const { data } = await api.post('/email/preview', req);
  return data;
}

export async function sendLetter(req: SendLetterRequest): Promise<EmailLog> {
  const { data } = await api.post('/email/send', req);
  return data;
}

// Logs
export async function getEmailLogs(params?: {
  employee_id?: string;
  page?: number;
  per_page?: number;
}): Promise<PaginatedResponse<EmailLog>> {
  const { data } = await api.get('/email/logs', { params });
  return data;
}
