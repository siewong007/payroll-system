import api from './client';
import type { Document, PaginatedResponse, CreateDocumentRequest, DocumentCategory } from '@/types';

export async function getDocuments(params?: {
  employee_id?: string;
  category_id?: string;
  status?: string;
  search?: string;
  page?: number;
  per_page?: number;
}): Promise<PaginatedResponse<Document>> {
  const { data } = await api.get('/documents', { params });
  return data;
}

export async function getDocument(id: string): Promise<Document> {
  const { data } = await api.get(`/documents/${id}`);
  return data;
}

export async function createDocument(req: CreateDocumentRequest): Promise<Document> {
  const { data } = await api.post('/documents', req);
  return data;
}

export async function updateDocument(id: string, req: Partial<CreateDocumentRequest> & { status?: string }): Promise<Document> {
  const { data } = await api.put(`/documents/${id}`, req);
  return data;
}

export async function deleteDocument(id: string): Promise<void> {
  await api.delete(`/documents/${id}`);
}

export async function getDocumentCategories(): Promise<DocumentCategory[]> {
  const { data } = await api.get('/documents/categories');
  return data;
}

export async function createDocumentCategory(req: { name: string; description?: string }): Promise<DocumentCategory> {
  const { data } = await api.post('/documents/categories', req);
  return data;
}

export async function getExpiringDocuments(days = 30): Promise<Document[]> {
  const { data } = await api.get('/documents/expiring', { params: { days } });
  return data;
}
