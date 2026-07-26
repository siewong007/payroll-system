import { beforeEach, describe, expect, it, vi } from 'vitest';

const apiMocks = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
  put: vi.fn(),
  delete: vi.fn(),
}));

vi.mock('@/api/client', () => ({ default: apiMocks }));

import * as approvals from '@/api/approvals';
import * as portal from '@/api/portal';
import * as geofence from '@/api/geofence';
import * as workSchedule from '@/api/workSchedule';
import * as totp from '@/api/totp';
import * as sessions from '@/api/sessions';
import * as notifications from '@/api/notifications';
import * as audit from '@/api/audit';
import * as settings from '@/api/settings';
import * as calendar from '@/api/calendar';
import * as teams from '@/api/teams';
import * as admin from '@/api/admin';

/** Captures the synthetic <a download> the browser-download helpers create. */
function captureDownload() {
  const realCreateElement = document.createElement.bind(document);
  const anchor = realCreateElement('a');
  const click = vi.fn();
  anchor.click = click;
  vi.spyOn(document, 'createElement').mockImplementation((tag: string) =>
    tag === 'a' ? anchor : realCreateElement(tag as 'div'),
  );
  return { anchor, click };
}

beforeEach(() => {
  apiMocks.get.mockReset().mockResolvedValue({ data: {}, headers: {} });
  apiMocks.post.mockReset().mockResolvedValue({ data: {} });
  apiMocks.put.mockReset().mockResolvedValue({ data: {} });
  apiMocks.delete.mockReset().mockResolvedValue({ data: {} });
  vi.restoreAllMocks();

  globalThis.URL.createObjectURL = vi.fn(() => 'blob:mock-url');
  globalThis.URL.revokeObjectURL = vi.fn();
});

describe('approvals API — leave', () => {
  it('filters the queue by status and unwraps the list', async () => {
    apiMocks.get.mockResolvedValue({ data: [{ id: 'leave-1' }] });

    await expect(approvals.getLeaveRequests('pending')).resolves.toEqual([{ id: 'leave-1' }]);
    expect(apiMocks.get).toHaveBeenCalledWith('/approvals/leave', {
      params: { status: 'pending' },
    });
  });

  it('omits the status filter when none is given', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await approvals.getLeaveRequests();

    expect(apiMocks.get).toHaveBeenCalledWith('/approvals/leave', { params: { status: undefined } });
  });

  it('carries approver notes on both approve and reject', async () => {
    await approvals.approveLeave('leave-1', 'Covered by Siti');
    expect(apiMocks.put).toHaveBeenLastCalledWith('/approvals/leave/leave-1/approve', {
      notes: 'Covered by Siti',
    });

    await approvals.rejectLeave('leave-1', 'Peak period');
    expect(apiMocks.put).toHaveBeenLastCalledWith('/approvals/leave/leave-1/reject', {
      notes: 'Peak period',
    });
  });

  it('separates cancel from delete — one is a state change, the other removes the row', async () => {
    await approvals.cancelLeaveRequest('leave-1');
    expect(apiMocks.put).toHaveBeenCalledWith('/approvals/leave/leave-1/cancel');

    await approvals.deleteLeaveRequest('leave-1');
    expect(apiMocks.delete).toHaveBeenCalledWith('/approvals/leave/leave-1');
  });
});

describe('approvals API — claims and overtime', () => {
  it('routes claim decisions to the claims path', async () => {
    await approvals.approveClaim('claim-1', 'Receipt verified');
    expect(apiMocks.put).toHaveBeenLastCalledWith('/approvals/claims/claim-1/approve', {
      notes: 'Receipt verified',
    });

    await approvals.rejectClaim('claim-1');
    expect(apiMocks.put).toHaveBeenLastCalledWith('/approvals/claims/claim-1/reject', {
      notes: undefined,
    });
  });

  it('routes overtime decisions to the overtime path', async () => {
    await approvals.approveOvertime('ot-1');
    expect(apiMocks.put).toHaveBeenLastCalledWith('/approvals/overtime/ot-1/approve', {
      notes: undefined,
    });

    await approvals.rejectOvertime('ot-1', 'Not pre-approved');
    expect(apiMocks.put).toHaveBeenLastCalledWith('/approvals/overtime/ot-1/reject', {
      notes: 'Not pre-approved',
    });
  });

  it('keeps the three approval queues on distinct endpoints', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await approvals.getLeaveRequests('pending');
    await approvals.getClaims('pending');
    await approvals.getOvertimeRequests('pending');

    const paths = apiMocks.get.mock.calls.map((call) => call[0]);
    expect(paths).toEqual(['/approvals/leave', '/approvals/claims', '/approvals/overtime']);
  });
});

describe('portal API', () => {
  it('reads self-service resources from the portal namespace', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await portal.getMyProfile();
    await portal.getMyPayslips();
    await portal.getLeaveRequests();
    await portal.getMyTeams();

    const paths = apiMocks.get.mock.calls.map((call) => call[0]);
    // Every self-service read must stay under /portal, which the backend scopes
    // to the caller's own employee record.
    expect(paths).toEqual([
      '/portal/profile',
      '/portal/payslips',
      '/portal/leave/requests',
      '/portal/teams',
    ]);
    expect(paths.every((path) => path.startsWith('/portal'))).toBe(true);
  });

  it('passes an optional year to balances, calendar, and holidays', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await portal.getLeaveBalances(2026);
    expect(apiMocks.get).toHaveBeenLastCalledWith('/portal/leave/balances', {
      params: { year: 2026 },
    });

    await portal.getTeamCalendar(2026, 7);
    expect(apiMocks.get).toHaveBeenLastCalledWith('/portal/team-calendar', {
      params: { year: 2026, month: 7 },
    });

    await portal.getPortalHolidays();
    expect(apiMocks.get).toHaveBeenLastCalledWith('/portal/holidays', {
      params: { year: undefined },
    });
  });

  it('submits a leave request with its computed day count', async () => {
    apiMocks.post.mockResolvedValue({ data: { id: 'leave-1' } });

    await portal.createLeaveRequest({
      leave_type_id: 'type-1',
      start_date: '2026-08-03',
      end_date: '2026-08-05',
      days: 3,
      reason: 'Family matters',
    });

    expect(apiMocks.post).toHaveBeenCalledWith('/portal/leave/requests', {
      leave_type_id: 'type-1',
      start_date: '2026-08-03',
      end_date: '2026-08-05',
      days: 3,
      reason: 'Family matters',
    });
  });

  it('uploads an attachment as multipart form data', async () => {
    apiMocks.post.mockResolvedValue({ data: { url: '/uploads/mc.pdf' } });
    const file = new File(['scan'], 'mc.pdf', { type: 'application/pdf' });

    await portal.uploadFile(file);

    const [path, body, config] = apiMocks.post.mock.calls[0];
    expect(path).toBe('/uploads');
    expect(body).toBeInstanceOf(FormData);
    expect((body as FormData).get('file')).toBe(file);
    expect(config).toEqual({ headers: { 'Content-Type': 'multipart/form-data' } });
  });

  it('downloads a payslip as a blob named after the payslip', async () => {
    const { anchor, click } = captureDownload();
    apiMocks.get.mockResolvedValue({ data: new Blob(['pdf']), headers: {} });

    await portal.downloadPayslipPdf('slip-7');

    expect(apiMocks.get).toHaveBeenCalledWith('/portal/payslips/slip-7/pdf', {
      responseType: 'blob',
    });
    expect(anchor.download).toBe('payslip_slip-7.pdf');
    expect(click).toHaveBeenCalledOnce();
    expect(globalThis.URL.revokeObjectURL).toHaveBeenCalledWith('blob:mock-url');
  });

  it('exports the leave calendar as an .ics file', async () => {
    const { anchor } = captureDownload();
    apiMocks.get.mockResolvedValue({ data: new Blob(['BEGIN:VCALENDAR']), headers: {} });

    await portal.exportLeaveIcs();

    expect(apiMocks.get).toHaveBeenCalledWith('/portal/leave/export-ics', {
      responseType: 'blob',
    });
    expect(anchor.download).toBe('leave_calendar.ics');
  });
});

describe('geofence API', () => {
  it('creates a location and lets the server apply the default radius', async () => {
    apiMocks.post.mockResolvedValue({ data: { id: 'loc-1' } });

    await geofence.createLocation({ name: 'HQ', latitude: 3.1576, longitude: 101.7118 });

    // radius_meters is omitted, so the backend's 200 m default applies.
    expect(apiMocks.post).toHaveBeenCalledWith('/geofence/locations', {
      name: 'HQ',
      latitude: 3.1576,
      longitude: 101.7118,
    });
  });

  it('deactivates a location through a partial update', async () => {
    await geofence.updateLocation('loc-1', { is_active: false });

    expect(apiMocks.put).toHaveBeenCalledWith('/geofence/locations/loc-1', { is_active: false });
  });

  it('reads and writes the enforcement mode', async () => {
    apiMocks.get.mockResolvedValue({ data: { mode: 'warn' } });
    await expect(geofence.getGeofenceMode()).resolves.toEqual({ mode: 'warn' });

    await geofence.setGeofenceMode('enforce');
    expect(apiMocks.put).toHaveBeenCalledWith('/geofence/mode', { mode: 'enforce' });
  });
});

describe('work schedule API', () => {
  it('separates the default-schedule endpoint from a schedule by id', async () => {
    apiMocks.get.mockResolvedValue({ data: { schedule: null } });

    await workSchedule.getDefaultSchedule();
    expect(apiMocks.get).toHaveBeenCalledWith('/work-schedules/default');

    await workSchedule.upsertDefaultSchedule({ start_time: '09:00', end_time: '18:00' });
    expect(apiMocks.put).toHaveBeenLastCalledWith('/work-schedules/default', {
      start_time: '09:00',
      end_time: '18:00',
    });

    await workSchedule.updateWorkSchedule('sched-1', { grace_minutes: 10 });
    expect(apiMocks.put).toHaveBeenLastCalledWith('/work-schedules/sched-1', {
      grace_minutes: 10,
    });
  });

  it('sends a zero grace period rather than dropping it as falsy', async () => {
    // grace_minutes: 0 means "no grace at all"; losing it would silently
    // restore the server-side default and mark nobody late.
    await workSchedule.upsertDefaultSchedule({
      start_time: '09:00',
      end_time: '18:00',
      grace_minutes: 0,
    });

    expect(apiMocks.put).toHaveBeenCalledWith('/work-schedules/default', {
      start_time: '09:00',
      end_time: '18:00',
      grace_minutes: 0,
    });
  });
});

describe('two-factor API', () => {
  it('runs setup as a begin/confirm pair', async () => {
    apiMocks.post.mockResolvedValue({ data: { secret: 'BASE32', otpauth_url: 'otpauth://' } });
    await totp.totpSetupBegin();
    expect(apiMocks.post).toHaveBeenLastCalledWith('/auth/2fa/setup/begin');

    apiMocks.post.mockResolvedValue({ data: { backup_codes: ['a', 'b'] } });
    await expect(totp.totpSetupConfirm('123456')).resolves.toEqual({ backup_codes: ['a', 'b'] });
    expect(apiMocks.post).toHaveBeenLastCalledWith('/auth/2fa/setup/confirm', { code: '123456' });
  });

  it('requires the current password to disable 2FA or reissue backup codes', async () => {
    await totp.totpDisable('current-password');
    expect(apiMocks.post).toHaveBeenLastCalledWith('/auth/2fa/disable', {
      password: 'current-password',
    });

    apiMocks.post.mockResolvedValue({ data: { backup_codes: [] } });
    await totp.totpRegenerateBackupCodes('current-password');
    expect(apiMocks.post).toHaveBeenLastCalledWith('/auth/2fa/backup-codes/regenerate', {
      password: 'current-password',
    });
  });

  it('completes a pending login challenge with the mfa token, not a session', async () => {
    apiMocks.post.mockResolvedValue({ data: { token: 'jwt', user: { id: 'user-1' } } });

    await totp.verifyTwoFactorLogin('mfa-token-abc', '654321');

    // The caller is not yet authenticated here, so the short-lived mfa_token is
    // the only thing tying this call to the pending login.
    expect(apiMocks.post).toHaveBeenCalledWith('/auth/2fa/verify', {
      mfa_token: 'mfa-token-abc',
      code: '654321',
    });
  });
});

describe('sessions API', () => {
  it('revokes one session by id and all others through a distinct endpoint', async () => {
    apiMocks.get.mockResolvedValue({ data: [{ id: 'sess-1', current: true }] });
    await sessions.getSessions();
    expect(apiMocks.get).toHaveBeenCalledWith('/auth/sessions');

    await sessions.revokeSession('sess-2');
    expect(apiMocks.delete).toHaveBeenLastCalledWith('/auth/sessions/sess-2');

    // "Sign out everywhere else" must not collide with the by-id route.
    await sessions.revokeOtherSessions();
    expect(apiMocks.delete).toHaveBeenLastCalledWith('/auth/sessions/others');
  });
});

describe('notifications API', () => {
  it('defaults to all notifications with a bounded page size', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await notifications.getNotifications();

    expect(apiMocks.get).toHaveBeenCalledWith('/notifications', {
      params: { unread_only: false, limit: 50 },
    });
  });

  it('narrows to unread with an explicit limit', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await notifications.getNotifications(true, 10);

    expect(apiMocks.get).toHaveBeenCalledWith('/notifications', {
      params: { unread_only: true, limit: 10 },
    });
  });

  it('marks one and all as read through separate endpoints', async () => {
    await notifications.markAsRead('notif-1');
    expect(apiMocks.put).toHaveBeenLastCalledWith('/notifications/notif-1/read');

    await notifications.markAllRead();
    expect(apiMocks.put).toHaveBeenLastCalledWith('/notifications/read-all');
  });
});

describe('audit API', () => {
  it('forwards the full filter set including pagination', async () => {
    apiMocks.get.mockResolvedValue({ data: { data: [], total: 0, page: 1, per_page: 25 } });

    const query = {
      entity_type: 'payroll_run',
      action: 'approve',
      user_id: 'user-1',
      start_date: '2026-07-01',
      end_date: '2026-07-31',
      page: 2,
      per_page: 25,
    };
    await audit.getAuditLogs(query);

    expect(apiMocks.get).toHaveBeenCalledWith('/audit-logs', { params: query });
  });
});

describe('settings API', () => {
  it('omits params entirely when no category is given', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await settings.getSettings();
    expect(apiMocks.get).toHaveBeenLastCalledWith('/settings', { params: undefined });

    await settings.getSettings('attendance');
    expect(apiMocks.get).toHaveBeenLastCalledWith('/settings', {
      params: { category: 'attendance' },
    });
  });

  it('addresses a single setting by category and key', async () => {
    apiMocks.get.mockResolvedValue({ data: { value: true } });
    await settings.getSetting('attendance', 'geofence_mode');
    expect(apiMocks.get).toHaveBeenCalledWith('/settings/attendance/geofence_mode');

    await settings.updateSetting('attendance', 'geofence_mode', 'enforce');
    expect(apiMocks.put).toHaveBeenCalledWith('/settings/attendance/geofence_mode', {
      value: 'enforce',
    });
  });

  it('wraps a bulk update in a settings envelope', async () => {
    const updates = [
      { category: 'attendance', key: 'grace_minutes', value: 10 },
      { category: 'payroll', key: 'auto_pcb', value: false },
    ];

    await settings.bulkUpdateSettings(updates as Parameters<typeof settings.bulkUpdateSettings>[0]);

    expect(apiMocks.put).toHaveBeenCalledWith('/settings', { settings: updates });
  });

  it('preserves a false setting value rather than dropping it', async () => {
    await settings.updateSetting('payroll', 'auto_pcb', false);

    expect(apiMocks.put).toHaveBeenCalledWith('/settings/payroll/auto_pcb', { value: false });
  });
});

describe('calendar API', () => {
  it('imports holidays from a URL and from an uploaded file by different routes', async () => {
    apiMocks.post.mockResolvedValue({ data: [] });

    await calendar.importIcs('https://example.test/holidays.ics');
    expect(apiMocks.post).toHaveBeenLastCalledWith('/calendar/import-ics', {
      url: 'https://example.test/holidays.ics',
    });

    const file = new File(['BEGIN:VCALENDAR'], 'holidays.ics', { type: 'text/calendar' });
    await calendar.importIcsFile(file);

    const [path, body, config] = apiMocks.post.mock.calls[1];
    expect(path).toBe('/calendar/import-ics-file');
    expect(body).toBeInstanceOf(FormData);
    expect((body as FormData).get('file')).toBe(file);
    expect(config).toEqual({ headers: { 'Content-Type': 'multipart/form-data' } });
  });

  it('reads a month calendar by year and month', async () => {
    apiMocks.get.mockResolvedValue({ data: {} });

    await calendar.getMonthCalendar(2026, 7);

    expect(apiMocks.get).toHaveBeenCalledWith('/calendar/month', { params: { year: 2026, month: 7 } });
  });
});

describe('teams API', () => {
  it('nests member operations under the team', async () => {
    apiMocks.get.mockResolvedValue({ data: [] });

    await teams.getTeamMembers('team-1');
    expect(apiMocks.get).toHaveBeenCalledWith('/teams/team-1/members');

    apiMocks.post.mockResolvedValue({ data: {} });
    await teams.addTeamMember('team-1', { employee_id: 'emp-1' } as Parameters<
      typeof teams.addTeamMember
    >[1]);
    expect(apiMocks.post).toHaveBeenCalledWith('/teams/team-1/members', { employee_id: 'emp-1' });

    await teams.removeTeamMember('team-1', 'emp-1');
    expect(apiMocks.delete).toHaveBeenCalledWith('/teams/team-1/members/emp-1');
  });
});

describe('admin API', () => {
  it('keeps the public password-reset endpoints off the admin namespace', async () => {
    await admin.forgotPassword('person@example.com');
    expect(apiMocks.post).toHaveBeenLastCalledWith('/auth/forgot-password', {
      email: 'person@example.com',
    });

    await admin.resetPassword('reset-token', 'NewStrongPass1');
    expect(apiMocks.post).toHaveBeenLastCalledWith('/auth/reset-password', {
      token: 'reset-token',
      new_password: 'NewStrongPass1',
    });
  });

  it('scopes a user list to one company when asked', async () => {
    apiMocks.get.mockResolvedValue({ data: { data: [], total: 0, page: 1, per_page: 20 } });

    await admin.listUsers({ companyId: 'company-2', page: 2, perPage: 20 });

    const [path, config] = apiMocks.get.mock.calls[0];
    expect(path).toBe('/admin/users');
    expect(config).toEqual({
      params: { company_id: 'company-2', search: undefined, page: 2, per_page: 20 },
    });
  });

  it('omits every filter when listing users unfiltered', async () => {
    apiMocks.get.mockResolvedValue({ data: { data: [], total: 0, page: 1, per_page: 20 } });

    await admin.listUsers();

    const [, config] = apiMocks.get.mock.calls[0];
    expect(config).toEqual({
      params: { company_id: undefined, search: undefined, page: undefined, per_page: undefined },
    });
  });

  it('drops a whitespace-only search term rather than sending it', async () => {
    apiMocks.get.mockResolvedValue({ data: { data: [], total: 0, page: 1, per_page: 20 } });

    await admin.listUsers({ search: '   ' });

    const [, config] = apiMocks.get.mock.calls[0];
    expect((config as { params: { search?: string } }).params.search).toBeUndefined();
  });

  it('creates, updates and deletes users against the admin routes', async () => {
    apiMocks.post.mockResolvedValue({ data: {} });
    apiMocks.put.mockResolvedValue({ data: {} });
    apiMocks.delete.mockResolvedValue({ data: {} });

    await admin.createUser({
      email: 'new@example.com',
      password: 'Str0ngPassword',
      full_name: 'New Person',
      roles: ['finance'],
      company_ids: ['company-1'],
    });
    expect(apiMocks.post).toHaveBeenLastCalledWith('/admin/users', expect.objectContaining({
      email: 'new@example.com',
    }));

    await admin.updateUser('user-1', { full_name: 'Renamed' });
    expect(apiMocks.put).toHaveBeenLastCalledWith('/admin/users/user-1', { full_name: 'Renamed' });

    await admin.deleteUser('user-1');
    expect(apiMocks.delete).toHaveBeenLastCalledWith('/admin/users/user-1');
  });
});
