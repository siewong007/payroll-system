import api from './client';
import type { Employee, LeaveType, LeaveBalance, LeaveRequest, MyPayslip, Holiday, TeamLeaveEntry, Team, OvertimeApplication, CreateOvertimeRequest } from '@/types';

// Profile
export async function getMyProfile(): Promise<Employee> {
  const { data } = await api.get('/portal/profile');
  return data;
}

// Payslips
export async function getMyPayslips(): Promise<MyPayslip[]> {
  const { data } = await api.get('/portal/payslips');
  return data;
}

// Leave
export async function getLeaveTypes(): Promise<LeaveType[]> {
  const { data } = await api.get('/portal/leave/types');
  return data;
}

export async function getLeaveBalances(year?: number): Promise<LeaveBalance[]> {
  const { data } = await api.get('/portal/leave/balances', { params: { year } });
  return data;
}

export async function getLeaveRequests(): Promise<LeaveRequest[]> {
  const { data } = await api.get('/portal/leave/requests');
  return data;
}

export async function createLeaveRequest(req: {
  leave_type_id: string;
  start_date: string;
  end_date: string;
  days: number;
  reason?: string;
  attachment_url?: string;
  attachment_name?: string;
}): Promise<LeaveRequest> {
  const { data } = await api.post('/portal/leave/requests', req);
  return data;
}

export async function cancelLeaveRequest(id: string): Promise<void> {
  await api.put(`/portal/leave/requests/${id}/cancel`);
}

export async function deleteLeaveRequest(id: string): Promise<void> {
  await api.delete(`/portal/leave/requests/${id}`);
}

// File Upload
export async function uploadFile(file: File): Promise<{ url: string; file_name: string; size: number }> {
  const formData = new FormData();
  formData.append('file', file);
  const { data } = await api.post('/uploads', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
  });
  return data;
}

// Overtime
export async function getOvertimeApplications(): Promise<OvertimeApplication[]> {
  const { data } = await api.get('/portal/overtime');
  return data;
}

export async function createOvertimeApplication(req: CreateOvertimeRequest): Promise<OvertimeApplication> {
  const { data } = await api.post('/portal/overtime', req);
  return data;
}

export async function cancelOvertimeApplication(id: string): Promise<void> {
  await api.put(`/portal/overtime/${id}/cancel`);
}

export async function deleteOvertimeApplication(id: string): Promise<void> {
  await api.delete(`/portal/overtime/${id}`);
}

// My Teams
export async function getMyTeams(): Promise<Team[]> {
  const { data } = await api.get('/portal/teams');
  return data;
}

// Team Calendar
export async function getTeamCalendar(year?: number, month?: number): Promise<TeamLeaveEntry[]> {
  const { data } = await api.get('/portal/team-calendar', { params: { year, month } });
  return data;
}

// Portal Holidays
export async function getPortalHolidays(year?: number): Promise<Holiday[]> {
  const { data } = await api.get('/portal/holidays', { params: { year } });
  return data;
}

function isIosTouchDevice(): boolean {
  if (typeof navigator === 'undefined') return false;
  return (
    /iPad|iPhone|iPod/.test(navigator.userAgent) ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1) ||
    Boolean((navigator as { standalone?: boolean }).standalone)
  );
}

function triggerBlobDownload(data: BlobPart, filename: string, mimeType: string): void {
  const blob = data instanceof Blob && data.type ? data : new Blob([data], { type: mimeType });
  const url = window.URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.rel = 'noopener noreferrer';
  const ios = isIosTouchDevice();
  if (ios) {
    a.target = '_blank';
  }
  a.style.display = 'none';
  document.body?.appendChild(a);
  a.click();
  if (typeof a.remove === 'function') {
    a.remove();
  }
  if (ios) {
    window.setTimeout(() => window.URL.revokeObjectURL(url), 10_000);
  } else {
    window.URL.revokeObjectURL(url);
  }
}

// Payslip PDF Download
export async function downloadPayslipPdf(payslipId: string): Promise<void> {
  const res = await api.get(`/portal/payslips/${payslipId}/pdf`, { responseType: 'blob' });
  triggerBlobDownload(res.data, `payslip_${payslipId}.pdf`, 'application/pdf');
}

// Leave ICS Export
export async function exportLeaveIcs(): Promise<void> {
  const res = await api.get('/portal/leave/export-ics', { responseType: 'blob' });
  triggerBlobDownload(res.data, 'leave_calendar.ics', 'text/calendar;charset=utf-8');
}
