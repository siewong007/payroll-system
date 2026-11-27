import api from './client';
import { triggerBlobDownload } from '@/lib/download';
import type {
  AuthenticationResponseJSON,
  PublicKeyCredentialRequestOptionsJSON,
} from '@/lib/webauthn';
import type { StepUpCredentials } from '@/types';

export type AttendanceMethod = 'qr_code' | 'face_id' | 'qr_and_face_id';
export type DeviceBindingMode = 'none' | 'warn' | 'enforce';
export type PhotoCaptureMode = 'none' | 'optional' | 'required';

export interface AttendanceMethodResponse {
  method: AttendanceMethod;
  allow_company_override: boolean;
  is_company_override: boolean;
  /** When 'none', clients should not block on a GPS fix — the server ignores coordinates */
  geofence_mode: 'none' | 'warn' | 'enforce';
  /**
   * Office-network requirement. 'learn' observes without blocking or flagging.
   * Only 'enforce' can refuse a check-in. The browser cannot read the Wi-Fi
   * network itself — the server decides from the address it observes — so this
   * is only ever used to *explain* the requirement, never to pre-check it.
   */
  network_mode: 'none' | 'learn' | 'warn' | 'enforce';
  device_mode: DeviceBindingMode;
  device_auto_approve_first: boolean;
  photo_mode: PhotoCaptureMode;
  photo_retention_days: number;
  /** IANA timezone the company's attendance days are bucketed in */
  timezone: string;
  /** Advisory default shift start time (`HH:MM:SS`) */
  shift_start_time: string | null;
  /** Advisory default shift grace minutes */
  grace_minutes: number | null;
}

export interface QrTokenResponse {
  token: string;
  expires_at: string;
  scan_url: string;
  /** Token lifetime in seconds — use for progress bar calculations */
  ttl_seconds: number;
}

export interface DeviceProofRequest {
  device_id: string;
  challenge_id: string;
  signature: string;
}

export interface AttendanceCheckOptions {
  device_proof?: DeviceProofRequest;
  photo_base64?: string;
}

export interface AttendanceRecord {
  id: string;
  company_id: string;
  employee_id: string;
  check_in_at: string;
  check_out_at: string | null;
  method: 'qr_code' | 'face_id' | 'qr_and_face_id' | 'kiosk_photo' | 'manual';
  status: 'present' | 'late' | 'absent' | 'half_day';
  latitude: number | null;
  longitude: number | null;
  checkout_latitude: number | null;
  checkout_longitude: number | null;
  notes: string | null;
  qr_token_id: string | null;
  /** NULL for employee-initiated and cron-written rows; set on admin entries. */
  created_by: string | null;
  hours_worked: string | null;
  overtime_hours: string | null;
  is_outside_geofence: boolean | null;
  /** Null means the network was never evaluated — mode was off or only learning. */
  is_offsite_network: boolean | null;
  check_in_device_id?: string | null;
  check_out_device_id?: string | null;
  check_in_user_agent?: string | null;
  check_out_user_agent?: string | null;
  is_untrusted_device?: boolean | null;
  anomaly_flags: string[];
  check_in_photo_url?: string | null;
  check_out_photo_url?: string | null;
  check_in_photo_purged_at?: string | null;
  check_out_photo_purged_at?: string | null;
  created_at: string;
  updated_at: string;
}

export interface AttendanceRecordWithEmployee
  extends Omit<AttendanceRecord, 'qr_token_id' | 'created_by' | 'updated_at'> {
  employee_number: string;
  full_name: string;
  department: string | null;
  check_in_device_label: string | null;
  check_out_device_label: string | null;
  check_in_passkey_id?: string | null;
}

export interface AttendanceListQuery {
  employee_id?: string;
  date_from?: string;
  date_to?: string;
  status?: string;
  method?: string;
  /** Only sessions never checked out — stale-session triage */
  open_only?: boolean;
  /** Only check-ins recorded outside the geofence — off-site triage */
  outside_geofence_only?: boolean;
  /** Only check-ins recorded off the approved networks — same triage need */
  offsite_network_only?: boolean;
  /** Only records carrying one or more anomaly flags */
  flagged_only?: boolean;
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

export function setPlatformAttendanceMethod(
  method: string,
  allow_company_override: boolean,
  stepUp: StepUpCredentials,
) {
  return api
    .put('/admin/platform/attendance-method', { method, allow_company_override, ...stepUp })
    .then((r) => r.data);
}

export function setCompanyAttendanceMethod(method: string | null) {
  return api.put('/attendance/company-method', { method }).then(r => r.data);
}

// ─── QR Token ───

export function generateQrToken(): Promise<QrTokenResponse> {
  return api.post('/attendance/qr/generate').then(r => r.data);
}

// ─── Check In / Out ───

function withAttendanceOptions<T extends Record<string, unknown>>(
  base: T,
  options?: AttendanceCheckOptions,
): T & AttendanceCheckOptions {
  if (!options) return base;
  const body: T & AttendanceCheckOptions = { ...base };
  if (options.device_proof) body.device_proof = options.device_proof;
  if (options.photo_base64) body.photo_base64 = options.photo_base64;
  return body;
}

export function checkInQr(
  token: string,
  latitude?: number,
  longitude?: number,
  options?: AttendanceCheckOptions,
): Promise<AttendanceRecord> {
  return api
    .post('/attendance/check-in/qr', withAttendanceOptions({ token, latitude, longitude }, options))
    .then(r => r.data);
}

export interface FaceIdBeginResponse {
  challenge_id: string;
  options: { publicKey: PublicKeyCredentialRequestOptionsJSON };
}

/**
 * Start the Face ID (passkey) check-in ceremony. The assertion returned by the
 * browser must be sent back via `checkInFaceId` with this challenge_id — the
 * server verifies it before recording the check-in.
 */
export function beginFaceIdCheckIn(): Promise<FaceIdBeginResponse> {
  return api.post('/attendance/check-in/face-id/begin').then(r => r.data);
}

export function checkInFaceId(
  challenge_id: string,
  credential: AuthenticationResponseJSON,
  latitude?: number,
  longitude?: number,
  options?: AttendanceCheckOptions,
): Promise<AttendanceRecord> {
  return api
    .post(
      '/attendance/check-in/face-id',
      withAttendanceOptions({ challenge_id, credential, latitude, longitude }, options),
    )
    .then(r => r.data);
}

export function checkInQrFaceId(
  token: string,
  challenge_id: string,
  credential: AuthenticationResponseJSON,
  latitude?: number,
  longitude?: number,
  options?: AttendanceCheckOptions,
): Promise<AttendanceRecord> {
  return api
    .post(
      '/attendance/check-in/qr-face-id',
      withAttendanceOptions({ token, challenge_id, credential, latitude, longitude }, options),
    )
    .then(r => r.data);
}

export function checkOut(
  latitude?: number,
  longitude?: number,
  options?: AttendanceCheckOptions,
): Promise<AttendanceRecord> {
  return api
    .post('/attendance/check-out', withAttendanceOptions({ latitude, longitude }, options))
    .then(r => r.data);
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
  /** Explicitly reopen the session. Omitting check_out_at only means "keep existing". */
  clear_check_out?: boolean;
  /** Explicitly blank the notes. Same tri-state rationale as clear_check_out. */
  clear_notes?: boolean;
  /** Required — recorded in the audit trail, not on the record */
  reason: string;
}): Promise<AttendanceRecord> {
  return api.put(`/attendance/records/${id}`, data).then(r => r.data);
}

/**
 * Re-run auto-absent marking for a past date (idempotent). Covers days the
 * daily job missed while the API was down.
 */
export function runAbsentMarking(date: string): Promise<{ marked: number; date: string }> {
  return api.post('/attendance/absent-run', { date }).then(r => r.data);
}

// ─── Summary & Export ───

export interface AttendanceSummaryItem {
  employee_id: string;
  employee_number: string;
  full_name: string;
  department: string | null;
  present_days: number;
  late_days: number;
  absent_days: number;
  half_days: number;
  total_hours: string;
  overtime_hours: string;
  unchecked_out_days: number;
}

export interface AttendanceSummaryQuery {
  date_from: string;
  date_to: string;
  employee_id?: string;
  department?: string;
}

export function getAttendanceSummary(params: AttendanceSummaryQuery): Promise<AttendanceSummaryItem[]> {
  return api.get('/attendance/summary', { params }).then(r => r.data);
}

export async function downloadAttendanceCsv(params: {
  date_from?: string;
  date_to?: string;
  employee_id?: string;
  status?: string;
  method?: string;
  open_only?: boolean;
  outside_geofence_only?: boolean;
  offsite_network_only?: boolean;
  flagged_only?: boolean;
}): Promise<void> {
  const response = await api.get('/attendance/export', {
    params,
    responseType: 'blob',
  });
  triggerBlobDownload(response.data, 'attendance.csv', 'text/csv');
}

// ─── Bound Devices, Security Settings, Anomalies, Personal Kiosk QR ───

export type DeviceChallengePurpose = 'check_in' | 'check_out' | 'personal_qr';

export interface DeviceChallengeResponse {
  challenge_id: string;
  nonce: string;
  expires_at: string;
}

export interface EmployeeAttendanceDevice {
  id: string;
  company_id: string;
  employee_id: string;
  user_id: string;
  public_key_raw: string;
  public_key_fingerprint: string;
  device_label: string;
  user_agent: string | null;
  cookie_device_id: string | null;
  status: 'pending' | 'approved' | 'revoked';
  approved_by: string | null;
  approved_at: string | null;
  revoked_by: string | null;
  revoked_at: string | null;
  revoke_reason: string | null;
  last_used_at: string | null;
  created_at: string;
}

export interface EmployeeAttendanceDeviceWithEmployee {
  id: string;
  company_id: string;
  employee_id: string;
  employee_number: string;
  full_name: string;
  department: string | null;
  device_label: string;
  public_key_fingerprint: string;
  user_agent: string | null;
  status: 'pending' | 'approved' | 'revoked';
  approved_by: string | null;
  approved_at: string | null;
  revoked_by: string | null;
  revoked_at: string | null;
  revoke_reason: string | null;
  last_used_at: string | null;
  created_at: string;
}

export interface AttendanceSecuritySettings {
  device_mode: DeviceBindingMode;
  device_auto_approve_first: boolean;
  photo_mode: PhotoCaptureMode;
  photo_retention_days: number;
}

export interface SharedDeviceEmployeeEntry {
  employee_id: string;
  employee_number: string;
  full_name: string;
  check_in_at: string;
}

export interface SharedDeviceEvent {
  local_date: string;
  device_tag: string;
  employee_count: number;
  employees: SharedDeviceEmployeeEntry[];
}

export interface PairedCheckInCluster {
  employee_a_id: string;
  employee_a_number: string;
  employee_a_name: string;
  employee_b_id: string;
  employee_b_number: string;
  employee_b_name: string;
  paired_days: number;
  paired_checkout_days: number;
  co_located_days: number;
  avg_gap_seconds: number;
  last_paired_date: string;
}

export interface AttendanceAnomaliesReport {
  date_from: string;
  date_to: string;
  summary: {
    flagged_records: number;
    shared_device_incidents: number;
    paired_clusters: number;
    pending_devices: number;
  };
  shared_device_events: SharedDeviceEvent[];
  paired_checkins: PairedCheckInCluster[];
}

export interface PersonalQrResponse {
  token: string;
  qr_payload: string;
  action: 'check_in' | 'check_out' | string;
  expires_at: string;
  ttl_seconds: number;
}

export function issueDeviceChallenge(purpose: DeviceChallengePurpose): Promise<DeviceChallengeResponse> {
  return api.post('/attendance/devices/challenge', { purpose }).then(r => r.data);
}

export function enrollAttendanceDevice(data: {
  public_key_raw: string;
  device_label?: string;
}): Promise<EmployeeAttendanceDevice> {
  return api.post('/attendance/devices/enroll', data).then(r => r.data);
}

export function getMyAttendanceDevices(): Promise<EmployeeAttendanceDevice[]> {
  return api.get('/attendance/devices/my').then(r => r.data);
}

export function listCompanyAttendanceDevices(
  status?: 'pending' | 'approved' | 'revoked',
): Promise<EmployeeAttendanceDeviceWithEmployee[]> {
  return api.get('/attendance/devices', { params: status ? { status } : undefined }).then(r => r.data);
}

export function approveAttendanceDevice(id: string): Promise<EmployeeAttendanceDevice> {
  return api.post(`/attendance/devices/${id}/approve`).then(r => r.data);
}

export function revokeAttendanceDevice(
  id: string,
  reason?: string,
): Promise<EmployeeAttendanceDevice> {
  return api.post(`/attendance/devices/${id}/revoke`, { reason }).then(r => r.data);
}

export function getAttendanceSecuritySettings(): Promise<AttendanceSecuritySettings> {
  return api.get('/attendance/security-settings').then(r => r.data);
}

export function updateAttendanceSecuritySettings(
  data: Partial<AttendanceSecuritySettings>,
): Promise<AttendanceSecuritySettings> {
  return api.put('/attendance/security-settings', data).then(r => r.data);
}

export function getAttendanceAnomalies(params?: {
  date_from?: string;
  date_to?: string;
}): Promise<AttendanceAnomaliesReport> {
  return api.get('/attendance/anomalies', { params }).then(r => r.data);
}

export function createPersonalQr(data?: {
  action?: 'check_in' | 'check_out';
  device_proof?: DeviceProofRequest;
}): Promise<PersonalQrResponse> {
  return api.post('/attendance/personal-qr', data ?? {}).then(r => r.data);
}
