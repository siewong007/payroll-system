import api from './client';

export interface AttendanceMethodResponse {
  method: 'qr_code' | 'face_id';
  allow_company_override: boolean;
  is_company_override: boolean;
}

export interface QrTokenResponse {
  token: string;
  expires_at: string;
  scan_url: string;
}

export interface AttendanceRecord {
  id: string;
  company_id: string;
  employee_id: string;
  check_in_at: string;
  check_out_at: string | null;
  method: 'qr_code' | 'face_id' | 'manual';
  status: 'present' | 'late' | 'absent' | 'half_day';
  latitude: number | null;
  longitude: number | null;
  checkout_latitude: number | null;
  checkout_longitude: number | null;
  notes: string | null;
  qr_token_id: string | null;
  hours_worked: number | null;
  overtime_hours: number | null;
  is_outside_geofence: boolean | null;
  created_at: string;
  updated_at: string;
}

export interface AttendanceRecordWithEmployee extends AttendanceRecord {
  employee_number: string;
  full_name: string;
  department: string | null;
}

export interface AttendanceListQuery {
  employee_id?: string;
  date_from?: string;
  date_to?: string;
  status?: string;
  method?: string;
  page?: number;
  per_page?: number;
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

// ─── Method Control ───

export function getAttendanceMethod(): Promise<AttendanceMethodResponse> {
  return api.get('/attendance/method').then(r => r.data);
}

export function getPlatformAttendanceMethod(): Promise<{ method: string; allow_company_override: boolean }> {
  return api.get('/admin/platform/attendance-method').then(r => r.data);
}

export function setPlatformAttendanceMethod(method: string, allow_company_override: boolean) {
  return api.put('/admin/platform/attendance-method', { method, allow_company_override }).then(r => r.data);
}

export function setCompanyAttendanceMethod(method: string | null) {
  return api.put('/attendance/company-method', { method }).then(r => r.data);
}

// ─── QR Token ───

export function generateQrToken(): Promise<QrTokenResponse> {
  return api.post('/attendance/qr/generate').then(r => r.data);
}

// ─── Check In / Out ───

export function checkInQr(token: string, latitude?: number, longitude?: number): Promise<AttendanceRecord> {
  return api.post('/attendance/check-in/qr', { token, latitude, longitude }).then(r => r.data);
}

export function checkInFaceId(credential_id: string, assertion: unknown, latitude?: number, longitude?: number): Promise<AttendanceRecord> {
  return api.post('/attendance/check-in/face-id', { credential_id, assertion, latitude, longitude }).then(r => r.data);
}

export function checkOut(latitude?: number, longitude?: number): Promise<AttendanceRecord> {
  return api.post('/attendance/check-out', { latitude, longitude }).then(r => r.data);
}

// ─── Portal ───

export function getMyTodayAttendance(): Promise<{ record: AttendanceRecord | null }> {
  return api.get('/attendance/my/today').then(r => r.data);
}

export function getMyAttendance(params?: AttendanceListQuery): Promise<PaginatedResponse<AttendanceRecord>> {
  return api.get('/attendance/my', { params }).then(r => r.data);
}

// ─── Admin ───

export function getAttendanceRecords(params?: AttendanceListQuery): Promise<PaginatedResponse<AttendanceRecordWithEmployee>> {
  return api.get('/attendance/records', { params }).then(r => r.data);
}

export function createManualAttendance(data: {
  employee_id: string;
  check_in_at: string;
  check_out_at?: string;
  status?: string;
  notes?: string;
}): Promise<AttendanceRecord> {
  return api.post('/attendance/manual', data).then(r => r.data);
}

export function updateAttendanceRecord(id: string, data: {
  check_in_at?: string;
  check_out_at?: string;
  status?: string;
  notes?: string;
}): Promise<AttendanceRecord> {
  return api.put(`/attendance/records/${id}`, data).then(r => r.data);
}
