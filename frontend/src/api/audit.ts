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

export interface AuditFilterOption {
  value: string;
  label: string;
}

export interface AuditFilterOptions {
  entity_types: AuditFilterOption[];
  actions: AuditFilterOption[];
}

/**
 * The values that actually appear in this company's audit trail.
 *
 * Read from the data rather than declared, so the dropdowns cannot drift from
 * what the backend writes. The page previously hardcoded 23 entity types and 12
 * actions; the list had gone stale in both directions — it offered `login`,
 * which is never written, and omitted 22 actions that are.
 */
export const getAuditFilterOptions = () =>
  api.get<AuditFilterOptions>('/audit-logs/filters').then((r) => r.data);
