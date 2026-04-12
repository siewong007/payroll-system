import api from './client';
import type { AuditLog, PaginatedResponse } from '@/types';

export interface AuditLogQuery {
  entity_type?: string;
  action?: string;
  user_id?: string;
  start_date?: string;
  end_date?: string;
  page?: number;
  per_page?: number;
}

export const getAuditLogs = (params: AuditLogQuery) =>
  api.get<PaginatedResponse<AuditLog>>('/audit-logs', { params }).then(r => r.data);
