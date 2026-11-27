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
  approveAttendanceDevice,
  checkInFaceId,
  checkInQr,
  checkInQrFaceId,
  checkOut,
  createPersonalQr,
  getAttendanceAnomalies,
  getAttendanceRecords,
  revokeAttendanceDevice,
  updateAttendanceSecuritySettings,
} from '@/api/attendance';
import { submitKioskCheckIn } from '@/api/kiosk';
import { classifyCheckInError, parsePersonalQrToken } from '@/lib/attendance';

beforeEach(() => {
  apiMocks.get.mockReset().mockResolvedValue({ data: {} });
  apiMocks.post.mockReset().mockResolvedValue({ data: {} });
  apiMocks.put.mockReset().mockResolvedValue({ data: {} });
});

describe('anti-buddy-punching error classification', () => {
  it('routes device approval & revocation messages to device-not-approved rather than expired-code', () => {
    expect(
      classifyCheckInError(
        'An approved attendance device is required to check in. Enroll this device and wait for HR approval.',
      ),
    ).toBe('device-not-approved');
    expect(
      classifyCheckInError('Your enrolled attendance device has been revoked by an administrator.'),
    ).toBe('device-not-approved');
    expect(classifyCheckInError('Your enrolled device is pending HR approval.')).toBe(
      'device-not-approved',
    );
    expect(classifyCheckInError('QR token has expired or been revoked')).toBe('expired-code');
  });

  it('routes missing live photo rejections to photo-required', () => {
    expect(classifyCheckInError('A live check-in photo is required for this company')).toBe(
      'photo-required',
    );
    expect(classifyCheckInError('A live check-out photo is required')).toBe('photo-required');
  });
});

describe('personal kiosk pass QR parsing', () => {
  it('extracts hex token from payroll-kiosk prefix and bare hex fallback', () => {
    expect(parsePersonalQrToken('payroll-kiosk:abcdef0123456789abcdef')).toBe(
      'abcdef0123456789abcdef',
    );
    expect(parsePersonalQrToken('abcdef0123456789abcdef')).toBe('abcdef0123456789abcdef');
    expect(parsePersonalQrToken('payroll-kiosk:short')).toBeNull();
    expect(parsePersonalQrToken('https://example.com/attendance/scan#token=123')).toBeNull();
    expect(parsePersonalQrToken('   ')).toBeNull();
  });
});

describe('attendance security & photo API contracts', () => {
  const cred = {
    id: 'pk-1',
    rawId: 'pk-1',
    type: 'public-key' as const,
    response: {
      authenticatorData: 'AA',
      clientDataJSON: 'BB',
      signature: 'CC',
      userHandle: null,
    },
  };
  const proof = {
    device_id: 'dev-1',
    challenge_id: 'ch-1',
    signature: 'sig-b64url',
  };

  it('includes device_proof and photo_base64 when provided and omits them when absent', async () => {
    await checkInQr('tok-1', 3.1, 101.6);
    expect(apiMocks.post).toHaveBeenLastCalledWith('/attendance/check-in/qr', {
      token: 'tok-1',
      latitude: 3.1,
      longitude: 101.6,
    });

    await checkInQr('tok-2', 3.1, 101.6, {
      device_proof: proof,
      photo_base64: 'data:image/jpeg;base64,/9j/4AAQ',
    });
    expect(apiMocks.post).toHaveBeenLastCalledWith('/attendance/check-in/qr', {
      token: 'tok-2',
      latitude: 3.1,
      longitude: 101.6,
      device_proof: proof,
      photo_base64: 'data:image/jpeg;base64,/9j/4AAQ',
    });

    await checkInQrFaceId('tok-3', 'ch-face', cred, 3.1, 101.6, { device_proof: proof });
    expect(apiMocks.post).toHaveBeenLastCalledWith('/attendance/check-in/qr-face-id', {
      token: 'tok-3',
      challenge_id: 'ch-face',
      credential: cred,
      latitude: 3.1,
      longitude: 101.6,
      device_proof: proof,
    });

    await checkInFaceId('ch-face-2', cred, undefined, undefined, {
      photo_base64: 'data:image/png;base64,iVBOR',
    });
    expect(apiMocks.post).toHaveBeenLastCalledWith('/attendance/check-in/face-id', {
      challenge_id: 'ch-face-2',
      credential: cred,
      latitude: undefined,
      longitude: undefined,
      photo_base64: 'data:image/png;base64,iVBOR',
    });

    await checkOut(3.1, 101.6, { device_proof: proof });
    expect(apiMocks.post).toHaveBeenLastCalledWith('/attendance/check-out', {
      latitude: 3.1,
      longitude: 101.6,
      device_proof: proof,
    });
  });

  it('calls security settings, device approval, anomaly, personal QR, and kiosk check-in endpoints', async () => {
    await updateAttendanceSecuritySettings({
      device_mode: 'enforce',
      device_auto_approve_first: true,
      photo_mode: 'required',
      photo_retention_days: 30,
    });
    expect(apiMocks.put).toHaveBeenLastCalledWith('/attendance/security-settings', {
      device_mode: 'enforce',
      device_auto_approve_first: true,
      photo_mode: 'required',
      photo_retention_days: 30,
    });

    await approveAttendanceDevice('dev-1');
    expect(apiMocks.post).toHaveBeenLastCalledWith('/attendance/devices/dev-1/approve');

    await revokeAttendanceDevice('dev-2', 'Lost phone');
    expect(apiMocks.post).toHaveBeenLastCalledWith('/attendance/devices/dev-2/revoke', {
      reason: 'Lost phone',
    });

    await getAttendanceRecords({ flagged_only: true });
    expect(apiMocks.get).toHaveBeenLastCalledWith('/attendance/records', {
      params: { flagged_only: true },
    });

    await getAttendanceAnomalies({ date_from: '2026-11-01', date_to: '2026-11-14' });
    expect(apiMocks.get).toHaveBeenLastCalledWith('/attendance/anomalies', {
      params: { date_from: '2026-11-01', date_to: '2026-11-14' },
    });

    await createPersonalQr({ action: 'check_in', device_proof: proof });
    expect(apiMocks.post).toHaveBeenLastCalledWith('/attendance/personal-qr', {
      action: 'check_in',
      device_proof: proof,
    });

    await submitKioskCheckIn('kiosk-secret', {
      personal_token: 'tok-personal',
      photo_base64: 'data:image/jpeg;base64,/9j/4AAQ',
    });
    expect(apiMocks.post).toHaveBeenLastCalledWith(
      '/attendance/kiosk/check-in',
      {
        personal_token: 'tok-personal',
        photo_base64: 'data:image/jpeg;base64,/9j/4AAQ',
      },
      { headers: { Authorization: 'Kiosk kiosk-secret' } },
    );
  });
});
