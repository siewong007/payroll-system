import api from './client';
import type {
  AdminCreateClaimRequest,
  AdminCreateLeaveRequest,
  AdminCreateOvertimeRequest,
  Claim,
  LeaveRequest,
  OvertimeApplication,
  OvertimeWithEmployee,
  UpdateClaimRequest,
  UpdateLeaveRequest,
  UpdateOvertimeRequest,
} from '@/types';

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

export const createLeaveRequest = (req: AdminCreateLeaveRequest) =>
  api.post<LeaveRequest>('/approvals/leave', req).then(r => r.data);

export const updateLeaveRequest = (id: string, req: UpdateLeaveRequest) =>
  api.put<LeaveRequest>(`/approvals/leave/${id}`, req).then(r => r.data);

export const deleteLeaveRequest = (id: string) =>
  api.delete(`/approvals/leave/${id}`).then(r => r.data);

export const approveLeave = (id: string, notes?: string) =>
  api.put(`/approvals/leave/${id}/approve`, { notes }).then(r => r.data);

export const rejectLeave = (id: string, notes?: string) =>
  api.put(`/approvals/leave/${id}/reject`, { notes }).then(r => r.data);

export const getClaims = (status?: string) =>
  api.get<ClaimWithEmployee[]>('/approvals/claims', { params: { status } }).then(r => r.data);

export const createClaim = (req: AdminCreateClaimRequest) =>
  api.post<Claim>('/approvals/claims', req).then(r => r.data);

export const updateClaim = (id: string, req: UpdateClaimRequest) =>
  api.put<ClaimWithEmployee>(`/approvals/claims/${id}`, req).then(r => r.data);

export const deleteClaim = (id: string) =>
  api.delete(`/approvals/claims/${id}`).then(r => r.data);

export const approveClaim = (id: string, notes?: string) =>
  api.put(`/approvals/claims/${id}/approve`, { notes }).then(r => r.data);

export const rejectClaim = (id: string, notes?: string) =>
  api.put(`/approvals/claims/${id}/reject`, { notes }).then(r => r.data);

// Overtime
export const getOvertimeRequests = (status?: string) =>
  api.get<OvertimeWithEmployee[]>('/approvals/overtime', { params: { status } }).then(r => r.data);

export const createOvertimeRequest = (req: AdminCreateOvertimeRequest) =>
  api.post<OvertimeApplication>('/approvals/overtime', req).then(r => r.data);

export const updateOvertimeRequest = (id: string, req: UpdateOvertimeRequest) =>
  api.put<OvertimeWithEmployee>(`/approvals/overtime/${id}`, req).then(r => r.data);

export const deleteOvertimeRequest = (id: string) =>
  api.delete(`/approvals/overtime/${id}`).then(r => r.data);

export const approveOvertime = (id: string, notes?: string) =>
  api.put(`/approvals/overtime/${id}/approve`, { notes }).then(r => r.data);

export const rejectOvertime = (id: string, notes?: string) =>
  api.put(`/approvals/overtime/${id}/reject`, { notes }).then(r => r.data);
