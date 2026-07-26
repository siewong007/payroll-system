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

export async function createDocument(req: CreateDocumentRequest): Promise<Document> {
  const { data } = await api.post('/documents', req);
  return data;
}

export async function deleteDocument(id: string): Promise<void> {
  await api.delete(`/documents/${id}`);
}

export async function getDocumentCategories(): Promise<DocumentCategory[]> {
  const { data } = await api.get('/documents/categories');
  return data;
}
