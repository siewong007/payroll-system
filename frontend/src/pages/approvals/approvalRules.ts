import type {
  ClaimWithEmployee,
  LeaveRequestWithEmployee,
  OvertimeWithEmployee,
} from '@/api/approvals';

export const canEditLeave = (request: LeaveRequestWithEmployee) => request.status === 'pending';
export const canCancelLeave = (request: LeaveRequestWithEmployee) =>
  ['pending', 'approved', 'rejected'].includes(request.status);
export const canDeleteLeave = (request: LeaveRequestWithEmployee) =>
  request.status === 'cancelled';

export const canEditClaim = (claim: ClaimWithEmployee) =>
  ['draft', 'pending', 'rejected'].includes(claim.status);
export const canCancelClaim = (claim: ClaimWithEmployee) =>
  ['pending', 'approved', 'rejected'].includes(claim.status);
export const canDeleteClaim = (claim: ClaimWithEmployee) =>
  ['draft', 'cancelled'].includes(claim.status);

export const canEditOvertime = (request: OvertimeWithEmployee) => request.status === 'pending';
export const canCancelOvertime = (request: OvertimeWithEmployee) =>
  ['pending', 'approved', 'rejected'].includes(request.status);
export const canDeleteOvertime = (request: OvertimeWithEmployee) =>
  request.status === 'cancelled';
