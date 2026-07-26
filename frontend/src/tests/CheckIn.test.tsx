import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { User } from '@/types';
import type { AttendanceRecord } from '@/api/attendance';
import { AuthContext, type AuthContextType } from '@/context/AuthContext';
import {
  classifyCheckInError, deriveTodayState, formatDuration, formatScheduleTime, formatZonedTime,
  parseScanToken, parseScheduleTime, shiftStanding,
} from '@/lib/attendance';
import { CheckInCard } from '@/components/attendance/CheckInCard';
import { userWithRoles } from './support/permissions';

// The camera is unavailable in jsdom; capture the decode callback so a scan
// can be driven from the test instead.
const camera = vi.hoisted(() => ({ onScan: null as ((text: string) => void) | null }));

vi.mock('html5-qrcode', () => {
  class FakeHtml5Qrcode {
    isScanning = false;
    start = vi.fn(async (_camera: unknown, _config: unknown, onSuccess: (text: string) => void) => {
      this.isScanning = true;
      camera.onScan = onSuccess;
      return null;
    });
    stop = vi.fn(async () => { this.isScanning = false; });
    clear = vi.fn();
    getRunningTrackCapabilities = vi.fn(() => ({}));
    applyVideoConstraints = vi.fn(async () => undefined);
  }
  return { Html5Qrcode: FakeHtml5Qrcode, Html5QrcodeSupportedFormats: { QR_CODE: 0 } };
});

vi.mock('@/api/attendance', () => ({
  getAttendanceMethod: vi.fn(),
  getMyTodayAttendance: vi.fn(),
  checkInQr: vi.fn(),
  beginFaceIdCheckIn: vi.fn(),
  checkInFaceId: vi.fn(),
  checkOut: vi.fn(),
}));

vi.mock('@/api/workSchedule', () => ({ getDefaultSchedule: vi.fn() }));
vi.mock('@/lib/webauthn', () => ({ getPasskeyCredential: vi.fn() }));

import {
  beginFaceIdCheckIn, checkInFaceId, checkInQr, checkOut, getAttendanceMethod, getMyTodayAttendance,
} from '@/api/attendance';
import { getDefaultSchedule } from '@/api/workSchedule';
import { getPasskeyCredential } from '@/lib/webauthn';

// ─── Pure helpers ─────────────────────────────────────────────────────────────

const baseRecord: AttendanceRecord = {
  id: 'r1',
  company_id: 'c1',
  employee_id: 'e1',
  check_in_at: '2026-07-27T01:02:00Z',
  check_out_at: null,
  method: 'qr_code',
  status: 'present',
  latitude: null,
  longitude: null,
  checkout_latitude: null,
  checkout_longitude: null,
  notes: null,
  qr_token_id: null,
  created_by: null,
  hours_worked: null,
  overtime_hours: null,
  is_outside_geofence: null,
  created_at: '2026-07-27T01:02:00Z',
  updated_at: '2026-07-27T01:02:00Z',
};

describe('deriveTodayState', () => {
  it('reports no record as not-checked-in', () => {
    expect(deriveTodayState(null)).toBe('not-checked-in');
  });

  it('reports an open session as checked-in', () => {
    expect(deriveTodayState(baseRecord)).toBe('checked-in');
  });

  it('reports a closed session as checked-out', () => {
    expect(deriveTodayState({ ...baseRecord, check_out_at: '2026-07-27T10:00:00Z' })).toBe('checked-out');
  });

  it('reports the auto-absent placeholder as absent, not checked-out', () => {
    // The cron writes a *closed* row at local midnight. Testing check_out_at
    // first would call this "checked out" and hide the check-in button on the
    // one day the employee most needs it.
    const placeholder: AttendanceRecord = {
      ...baseRecord,
      status: 'absent',
      method: 'manual',
      check_in_at: '2026-07-26T16:00:00Z',
      check_out_at: '2026-07-26T16:00:00Z',
    };
    expect(deriveTodayState(placeholder)).toBe('absent');
  });
});

describe('classifyCheckInError', () => {
  it('separates an off-site rejection from a missing permission', () => {
    // Both mention "location"; only one is fixed by granting permission.
    expect(classifyCheckInError("You are 350m from 'HQ'. Please check in from an approved office location."))
      .toBe('outside-geofence');
    expect(classifyCheckInError('You are outside all approved office locations.')).toBe('outside-geofence');
    expect(classifyCheckInError('Location is required for check-in. Please enable location services.'))
      .toBe('location-permission');
  });

  it('recognises a recorded duplicate', () => {
    expect(classifyCheckInError('You have already checked in today. Please check out first.'))
      .toBe('already-checked-in');
  });

  it('recognises a stale open session', () => {
    expect(classifyCheckInError('You have a check-in from 2026-07-26 that was never checked out. Check out from that session, or ask an administrator to correct it, before checking in today.'))
      .toBe('stale-session');
  });

  it('recognises a dead QR code and a missing passkey', () => {
    expect(classifyCheckInError('QR code has expired — please refresh the kiosk screen.')).toBe('expired-code');
    expect(classifyCheckInError('This QR code has been revoked — please refresh the kiosk screen.')).toBe('expired-code');
    expect(classifyCheckInError('No passkeys registered. Add a passkey in your profile to use Face ID check-in.'))
      .toBe('no-passkey');
  });

  it('falls back to generic for anything unrecognised', () => {
    expect(classifyCheckInError('Internal server error')).toBe('generic');
  });
});

describe('parseScanToken', () => {
  it('extracts the token from a kiosk URL', () => {
    expect(parseScanToken('https://payroll.example/attendance/scan?token=abc123def4567890'))
      .toEqual({ token: 'abc123def4567890' });
  });

  it('accepts a bare token for manual entry', () => {
    expect(parseScanToken('  abc123def4567890  ')).toEqual({ token: 'abc123def4567890' });
  });

  it('rejects a URL that carries no token', () => {
    const result = parseScanToken('https://example.com/');
    expect('error' in result).toBe(true);
  });

  it('rejects unparseable input', () => {
    expect('error' in parseScanToken('hello world')).toBe(true);
    expect('error' in parseScanToken('')).toBe(true);
  });
});

describe('shiftStanding', () => {
  const nineAm = 9 * 60;

  it('is early before the start time', () => {
    expect(shiftStanding(8 * 60 + 50, nineAm, 15)).toEqual({ kind: 'early', minutes: 10 });
  });

  it('is within grace up to and including the cutoff', () => {
    expect(shiftStanding(nineAm + 15, nineAm, 15).kind).toBe('grace');
  });

  it('is late one minute past the cutoff, matching the server rule', () => {
    expect(shiftStanding(nineAm + 16, nineAm, 15)).toEqual({ kind: 'late', minutes: 1 });
  });

  it('is unknown without a schedule', () => {
    expect(shiftStanding(nineAm, null, 15)).toEqual({ kind: 'unknown' });
  });
});

describe('parseScheduleTime and formatDuration', () => {
  it('parses HH:MM:SS and HH:MM', () => {
    expect(parseScheduleTime('09:00:00')).toBe(540);
    expect(parseScheduleTime('17:30')).toBe(1050);
    expect(parseScheduleTime('nonsense')).toBeNull();
    expect(parseScheduleTime(null)).toBeNull();
  });

  it('formats a shift start the same way recorded times are formatted', () => {
    // Both must come from the same locale path, or the card reads
    // "Shift starts 9:00 AM" next to "on the clock since 9:02 am".
    const start = formatScheduleTime('09:00:00');
    const recorded = formatZonedTime('2026-07-27T01:02:00Z', 'Asia/Kuala_Lumpur');
    expect(start).toMatch(/^9:00\s?[ap]m$/i);
    expect(recorded).toMatch(/^9:02\s?[ap]m$/i);
    // Same meridiem casing on both sides.
    expect(/AM|PM/.test(start ?? '')).toBe(/AM|PM/.test(recorded));
  });

  it('formats elapsed time', () => {
    expect(formatDuration(30_000)).toBe('just now');
    expect(formatDuration(48 * 60_000)).toBe('48m');
    expect(formatDuration((3 * 60 + 12) * 60_000)).toBe('3h 12m');
  });
});

// ─── CheckInCard ──────────────────────────────────────────────────────────────

const employee: User = userWithRoles(['employee'], {
  id: 'u1',
  email: 'staff@test.local',
  full_name: 'Siti Employee',
  company_id: 'c1',
  employee_id: 'e1',
});

function makeAuth(user: User | null): AuthContextType {
  return {
    user,
    token: 'test-token',
    login: vi.fn(),
    completeTwoFactorLogin: vi.fn(),
    logout: vi.fn(),
    switchCompany: vi.fn(),
    setSession: vi.fn(),
    isAuthenticated: true,
    isLoading: false,
  };
}

function renderCard(user: User | null = employee) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={makeAuth(user)}>
        <MemoryRouter initialEntries={['/portal/profile']}>
          <CheckInCard />
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>,
  );
}

const methodMock = vi.mocked(getAttendanceMethod);
const todayMock = vi.mocked(getMyTodayAttendance);
const scheduleMock = vi.mocked(getDefaultSchedule);

function setMethod(method: 'qr_code' | 'face_id', geofence: 'none' | 'warn' | 'enforce' = 'none') {
  methodMock.mockResolvedValue({
    method,
    allow_company_override: false,
    is_company_override: false,
    geofence_mode: geofence,
    timezone: 'Asia/Kuala_Lumpur',
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  camera.onScan = null;
  scheduleMock.mockResolvedValue({ schedule: null });
  todayMock.mockResolvedValue({ record: null });
  setMethod('qr_code');
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('CheckInCard', () => {
  it('checks in with a single tap at a Face ID company', async () => {
    setMethod('face_id');
    vi.mocked(beginFaceIdCheckIn).mockResolvedValue({
      challenge_id: 'ch1',
      options: { publicKey: { challenge: 'x' } },
    });
    vi.mocked(getPasskeyCredential).mockResolvedValue({ id: 'cred1' });
    vi.mocked(checkInFaceId).mockResolvedValue({ ...baseRecord, method: 'face_id' });

    renderCard();
    const button = await screen.findByRole('button', { name: /check in with face id/i });
    await userEvent.click(button);

    // One tap runs the whole ceremony — challenge, assertion, then the record.
    await waitFor(() => expect(checkInFaceId).toHaveBeenCalledTimes(1));
    expect(vi.mocked(checkInFaceId).mock.calls[0][0]).toBe('ch1');
    expect(checkInQr).not.toHaveBeenCalled();
  });

  it('opens the scanner and checks in without leaving the page at a QR company', async () => {
    setMethod('qr_code');
    vi.mocked(checkInQr).mockResolvedValue(baseRecord);

    renderCard();
    await userEvent.click(await screen.findByRole('button', { name: /^check in$/i }));

    const sheet = await screen.findByRole('dialog', { name: /scan attendance qr/i });
    expect(sheet).toBeInTheDocument();

    await waitFor(() => expect(camera.onScan).not.toBeNull());
    // The decode callback comes from the camera, outside React's batching.
    await act(async () => {
      camera.onScan?.('https://payroll.example/attendance/scan?token=abc123def4567890');
    });

    await waitFor(() => expect(checkInQr).toHaveBeenCalledWith('abc123def4567890', undefined, undefined));
    // The sheet closes on success rather than handing off to /attendance/scan.
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('does not branch on the check-in method before it has loaded', async () => {
    // A tap while `method` is undefined used to open the QR camera even at a
    // Face ID company, so the button stays disabled until the answer arrives.
    methodMock.mockReturnValue(new Promise(() => { /* never resolves */ }));

    renderCard();
    const button = await screen.findByRole('button', { name: /loading/i });
    expect(button).toBeDisabled();

    await userEvent.click(button);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(beginFaceIdCheckIn).not.toHaveBeenCalled();
  });

  it('still offers check-in when the day carries an auto-absent placeholder', async () => {
    todayMock.mockResolvedValue({
      record: { ...baseRecord, status: 'absent', method: 'manual', check_out_at: baseRecord.check_in_at },
    });

    renderCard();
    expect(await screen.findByRole('button', { name: /^check in$/i })).toBeEnabled();
    expect(screen.getByText(/marked absent today/i)).toBeInTheDocument();
  });

  it('shows a retry card instead of a check-in button when today is unknown', async () => {
    todayMock.mockRejectedValue(new Error('network down'));

    renderCard();
    expect(await screen.findByRole('button', { name: /retry/i })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /check in/i })).not.toBeInTheDocument();
  });

  it('requires a confirming tap before ending an open session', async () => {
    todayMock.mockResolvedValue({ record: baseRecord });
    vi.mocked(checkOut).mockResolvedValue({ ...baseRecord, check_out_at: '2026-07-27T10:00:00Z' });

    renderCard();
    await userEvent.click(await screen.findByRole('button', { name: /^check out$/i }));

    // A mistap must not end the session — re-check-in on the same day is blocked
    // server-side, so an accidental check-out strands the employee.
    expect(checkOut).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: /tap again to confirm/i }));
    await waitFor(() => expect(checkOut).toHaveBeenCalledTimes(1));
  });

  it('sends coordinates when the company enforces a geofence', async () => {
    setMethod('face_id', 'enforce');
    const getCurrentPosition = vi.fn((success: PositionCallback) =>
      success({ coords: { latitude: 3.1234, longitude: 101.6543 } } as GeolocationPosition));
    vi.stubGlobal('navigator', { ...navigator, geolocation: { getCurrentPosition } });

    vi.mocked(beginFaceIdCheckIn).mockResolvedValue({ challenge_id: 'ch1', options: { publicKey: { challenge: 'x' } } });
    vi.mocked(getPasskeyCredential).mockResolvedValue({ id: 'cred1' });
    vi.mocked(checkInFaceId).mockResolvedValue({ ...baseRecord, method: 'face_id' });

    renderCard();
    await userEvent.click(await screen.findByRole('button', { name: /check in with face id/i }));

    await waitFor(() => expect(checkInFaceId).toHaveBeenCalled());
    const call = vi.mocked(checkInFaceId).mock.calls[0];
    expect(call[2]).toBeCloseTo(3.1234);
    expect(call[3]).toBeCloseTo(101.6543);
  });

  it('does not pay for a GPS fix when geofencing is off', async () => {
    setMethod('face_id', 'none');
    const getCurrentPosition = vi.fn();
    vi.stubGlobal('navigator', { ...navigator, geolocation: { getCurrentPosition } });

    vi.mocked(beginFaceIdCheckIn).mockResolvedValue({ challenge_id: 'ch1', options: { publicKey: { challenge: 'x' } } });
    vi.mocked(getPasskeyCredential).mockResolvedValue({ id: 'cred1' });
    vi.mocked(checkInFaceId).mockResolvedValue({ ...baseRecord, method: 'face_id' });

    renderCard();
    await userEvent.click(await screen.findByRole('button', { name: /check in with face id/i }));

    await waitFor(() => expect(checkInFaceId).toHaveBeenCalled());
    expect(getCurrentPosition).not.toHaveBeenCalled();
  });

  it('tells an unlinked account to contact HR instead of offering check-in', async () => {
    renderCard({ ...employee, employee_id: null });
    expect(await screen.findByText(/isn't linked to an employee profile/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /check in/i })).not.toBeInTheDocument();
  });
});
