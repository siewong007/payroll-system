import { act, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const kioskMocks = vi.hoisted(() => ({ fetchKioskQr: vi.fn() }));

vi.mock('@/api/kiosk', () => ({ fetchKioskQr: kioskMocks.fetchKioskQr }));
// jsdom has no 2D canvas, and the QR bitmap is not what these tests are about.
vi.mock('qrcode', () => ({ default: { toCanvas: vi.fn() } }));

import { AttendanceKioskPublic } from '@/pages/attendance/AttendanceKioskPublic';

/** Monotonic clock the component reads through `performance.now()`. */
let clock = 0;

function renderKiosk() {
  return render(
    <MemoryRouter initialEntries={['/kiosk/secret-1']}>
      <Routes>
        <Route path="/kiosk/:kioskKey" element={<AttendanceKioskPublic />} />
      </Routes>
    </MemoryRouter>,
  );
}

async function advance(ms: number) {
  clock += ms;
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  clock = 0;
  vi.spyOn(performance, 'now').mockImplementation(() => clock);
  kioskMocks.fetchKioskQr.mockReset();
});

afterEach(() => {
  // Restore the spy before the fake timers, or the fake `performance.now` gets
  // put back onto the real object.
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('public kiosk QR lifecycle', () => {
  it('drives the countdown from ttl_seconds, ignoring a skewed device clock', async () => {
    // The tablet's clock runs five minutes fast, so the server's absolute
    // `expires_at` is already in the past the instant the token arrives. That
    // used to read as "expired", and the expiry effect minted again immediately
    // — for as long as the kiosk was powered on.
    kioskMocks.fetchKioskQr.mockResolvedValue({
      token: 'tok-1',
      scan_url: 'https://payroll.example/attendance/scan?token=tok-1',
      expires_at: new Date(Date.now() - 5 * 60_000).toISOString(),
      ttl_seconds: 300,
    });

    renderKiosk();
    await advance(0);

    expect(kioskMocks.fetchKioskQr).toHaveBeenCalledTimes(1);
    expect(screen.getByText('300s')).toBeInTheDocument();

    await advance(60_000);
    expect(screen.getByText('240s')).toBeInTheDocument();
    expect(kioskMocks.fetchKioskQr).toHaveBeenCalledTimes(1);

    // One refresh, armed a guard interval before the server deadline.
    await advance(240_000);
    expect(kioskMocks.fetchKioskQr).toHaveBeenCalledTimes(2);
  });

  it('falls back to a usable TTL when the server reports zero', async () => {
    kioskMocks.fetchKioskQr.mockResolvedValue({
      token: 'tok-1',
      scan_url: 'https://payroll.example/attendance/scan?token=tok-1',
      expires_at: new Date().toISOString(),
      ttl_seconds: 0,
    });

    renderKiosk();
    await advance(0);

    // A zero TTL must degrade to a short countdown, not to an instantly expired
    // token that re-mints on every tick.
    expect(screen.getByText('60s')).toBeInTheDocument();
    expect(kioskMocks.fetchKioskQr).toHaveBeenCalledTimes(1);
  });

  it('holds a short backoff retry to the minimum mint interval', async () => {
    kioskMocks.fetchKioskQr.mockRejectedValue(new Error('network down'));

    renderKiosk();
    await advance(0);

    expect(kioskMocks.fetchKioskQr).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/Retrying in 2s/)).toBeInTheDocument();

    // The backoff asks for 2 s; the floor defers it rather than dropping it, so
    // the kiosk still recovers but can never mint twice inside five seconds.
    await advance(2_500);
    expect(kioskMocks.fetchKioskQr).toHaveBeenCalledTimes(1);

    await advance(2_600);
    expect(kioskMocks.fetchKioskQr).toHaveBeenCalledTimes(2);
  });

  it('stops permanently on a revoked kiosk credential', async () => {
    kioskMocks.fetchKioskQr.mockRejectedValue({ response: { status: 401 } });

    renderKiosk();
    await advance(0);

    expect(screen.getByText(/This kiosk has been disconnected/)).toBeInTheDocument();

    await advance(600_000);
    expect(kioskMocks.fetchKioskQr).toHaveBeenCalledTimes(1);
  });
});
