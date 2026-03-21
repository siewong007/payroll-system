import api from './client';
import type { LeaveRequest, Claim, OvertimeWithEmployee } from '@/types';

export interface LeaveRequestWithEmployee extends LeaveRequest {
  employee_name: string | null;
  employee_number: string | null;
}

export interface ClaimWithEmployee extends Claim {
  employee_name: string | null;
  employee_number: string | null;
}

export type { OvertimeWithEmployee };

export const getLeaveRequests = (status?: string) =>
  api.get<LeaveRequestWithEmployee[]>('/approvals/leave', { params: { status } }).then(r => r.data);

export const approveLeave = (id: string, notes?: string) =>
  api.put(`/approvals/leave/${id}/approve`, { notes }).then(r => r.data);

export const rejectLeave = (id: string, notes?: string) =>
  api.put(`/approvals/leave/${id}/reject`, { notes }).then(r => r.data);

export const getClaims = (status?: string) =>
  api.get<ClaimWithEmployee[]>('/approvals/claims', { params: { status } }).then(r => r.data);

export const approveClaim = (id: string, notes?: string) =>
  api.put(`/approvals/claims/${id}/approve`, { notes }).then(r => r.data);

export const rejectClaim = (id: string, notes?: string) =>
  api.put(`/approvals/claims/${id}/reject`, { notes }).then(r => r.data);

// Overtime
export const getOvertimeRequests = (status?: string) =>
  api.get<OvertimeWithEmployee[]>('/approvals/overtime', { params: { status } }).then(r => r.data);

export const approveOvertime = (id: string, notes?: string) =>
  api.put(`/approvals/overtime/${id}/approve`, { notes }).then(r => r.data);

export const rejectOvertime = (id: string, notes?: string) =>
  api.put(`/approvals/overtime/${id}/reject`, { notes }).then(r => r.data);
