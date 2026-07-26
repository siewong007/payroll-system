import { beforeEach, describe, expect, it, vi } from 'vitest';

const apiMocks = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
  put: vi.fn(),
}));

vi.mock('@/api/client', () => ({
  default: apiMocks,
}));

import {
  beginFaceIdCheckIn,
  checkInFaceId,
  getAttendanceRecords,
  updateAttendanceRecord,
} from '@/api/attendance';
import { ATTENDANCE_VIEW_ROLES } from '@/lib/roles';

beforeEach(() => {
  apiMocks.get.mockReset().mockResolvedValue({ data: {} });
  apiMocks.post.mockReset().mockResolvedValue({ data: {} });
  apiMocks.put.mockReset().mockResolvedValue({ data: {} });
});

describe('attendance corrections', () => {
  it('sends explicit clear flags so blanking a field is not read as "keep existing"', async () => {
    await updateAttendanceRecord('record-1', {
      status: 'present',
      reason: 'Employee forgot to check out',
      clear_check_out: true,
      clear_notes: true,
    });

    expect(apiMocks.put).toHaveBeenCalledWith(
      '/attendance/records/record-1',
      expect.objectContaining({
        clear_check_out: true,
        clear_notes: true,
        reason: 'Employee forgot to check out',
      }),
    );
  });

  it('always carries a correction reason for the audit trail', async () => {
    await updateAttendanceRecord('record-2', { status: 'late', reason: 'Approved by manager' });

    const body = apiMocks.put.mock.calls[0][1] as { reason?: string };
    expect(body.reason).toBe('Approved by manager');
  });
});

describe('attendance record queries', () => {
  it('passes the open-only filter through for stale-session triage', async () => {
    apiMocks.get.mockResolvedValue({ data: { data: [], total: 0, page: 1, per_page: 50, total_pages: 0 } });

    await getAttendanceRecords({ open_only: true, employee_id: 'emp-1' });

    expect(apiMocks.get).toHaveBeenCalledWith('/attendance/records', {
      params: { open_only: true, employee_id: 'emp-1' },
    });
  });
});

describe('face id check-in', () => {
  it('runs a begin/complete ceremony so the server can verify the assertion', async () => {
    apiMocks.post.mockResolvedValue({
      data: { challenge_id: 'challenge-1', options: { publicKey: { challenge: 'abc' } } },
    });

    const begun = await beginFaceIdCheckIn();
    expect(apiMocks.post).toHaveBeenCalledWith('/attendance/check-in/face-id/begin');
    expect(begun.challenge_id).toBe('challenge-1');

    apiMocks.post.mockResolvedValue({ data: { id: 'record-1' } });
    const credential = {
      id: 'cred-1',
      rawId: 'cred-1',
      type: 'public-key',
      response: {
        authenticatorData: 'a',
        clientDataJSON: 'b',
        signature: 'c',
        userHandle: null,
      },
    };

    await checkInFaceId('challenge-1', credential, 3.1, 101.6);

    // The challenge id must travel with the assertion: without it the server
    // has no ceremony to verify against and the biometric claim is unproven.
    expect(apiMocks.post).toHaveBeenLastCalledWith('/attendance/check-in/face-id', {
      challenge_id: 'challenge-1',
      credential,
      latitude: 3.1,
      longitude: 101.6,
    });
  });
});

describe('attendance view roles', () => {
  it('is an allow-list that excludes self-service employees', () => {
    // Mirrors the backend's require_attendance_viewer. A new role must opt in
    // rather than inherit access, so employee must never appear here.
    expect(ATTENDANCE_VIEW_ROLES).not.toContain('employee');
    expect(ATTENDANCE_VIEW_ROLES).toEqual(
      expect.arrayContaining(['super_admin', 'admin', 'hr_manager', 'payroll_admin', 'finance', 'exec']),
    );
  });
});
