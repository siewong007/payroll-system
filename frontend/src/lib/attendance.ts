import type { AttendanceRecord } from '@/api/attendance';

/**
 * Shared attendance helpers.
 *
 * These live in a `.ts` module rather than beside the components that use them
 * because `react-refresh/only-export-components` is error-severity here — a
 * `.tsx` file may not export a plain function. Keeping them here also means the
 * three check-in surfaces (portal home card, My Attendance, the QR deep-link
 * landing page) agree on geolocation policy and error classification instead of
 * each carrying its own copy.
 */

// ─── Today's state ────────────────────────────────────────────────────────────

export type TodayState = 'not-checked-in' | 'checked-in' | 'checked-out' | 'absent';

/**
 * Collapse today's record into the state the UI acts on.
 *
 * The order matters. The auto-absent cron writes a *closed* placeholder row
 * (check_in_at = local midnight, check_out_at = the same instant, method
 * 'manual', status 'absent') for anyone who never showed up, and
 * `GET /attendance/my/today` returns it like any other record. Testing
 * `check_out_at` first therefore reports "checked out" and hides the check-in
 * button on the one day the employee most needs it. A real check-in supersedes
 * the placeholder server-side — it is deleted in the same transaction as the
 * insert — so offering the button here is correct, not a duplicate.
 */
export function deriveTodayState(record: AttendanceRecord | null | undefined): TodayState {
  if (!record) return 'not-checked-in';
  if (record.status === 'absent') return 'absent';
  return record.check_out_at ? 'checked-out' : 'checked-in';
}

// ─── Server error classification ──────────────────────────────────────────────

export type CheckInFailure =
  | 'already-checked-in'
  | 'stale-session'
  | 'outside-geofence'
  | 'location-permission'
  | 'expired-code'
  | 'no-passkey'
  | 'not-enabled'
  | 'generic';

/**
 * Map a server error message onto the recovery the employee actually needs.
 *
 * The backend returns prose in `{"error": …}`, so matching is by phrase. Order
 * is significant: "Please check in from an approved office location" contains
 * the word "location" but must NOT be read as a permission problem — telling
 * someone standing 300 m away to enable location services sends them round a
 * loop that cannot succeed.
 */
export function classifyCheckInError(message: string): CheckInFailure {
  const m = message.toLowerCase();
  if (/already checked in/.test(m)) return 'already-checked-in';
  if (/never checked out|active check-in|more than 24 hours old/.test(m)) return 'stale-session';
  if (/approved office location/.test(m)) return 'outside-geofence';
  if (/location is required|enable location services/.test(m)) return 'location-permission';
  if (/revoked|expired/.test(m)) return 'expired-code';
  if (/no passkeys registered/.test(m)) return 'no-passkey';
  if (/not enabled for this company/.test(m)) return 'not-enabled';
  return 'generic';
}

// ─── QR payload ───────────────────────────────────────────────────────────────

/** A bare token pasted or encoded without the surrounding URL. */
const BARE_TOKEN = /^[0-9a-f]{16,}$/i;

/**
 * Pull the attendance token out of a scanned payload.
 *
 * The kiosk encodes an absolute `…/attendance/scan?token=<hex>` URL, but a
 * bare token is accepted too so the manual-entry fallback can reuse this. The
 * token itself is not validated beyond "looks like one" — the server is the
 * authority on whether it is live, revoked or from another company.
 */
export function parseScanToken(decoded: string): { token: string } | { error: string } {
  const raw = decoded.trim();
  if (!raw) return { error: 'Nothing was scanned. Try again.' };

  if (BARE_TOKEN.test(raw)) return { token: raw };

  try {
    const token = new URL(raw).searchParams.get('token')?.trim();
    if (token) return { token };
    return { error: "That's a valid QR code, but not an attendance one. Scan the code on the kiosk screen." };
  } catch {
    return { error: "That doesn't look like an attendance QR code. Scan the code on the kiosk screen." };
  }
}

// ─── Geolocation ──────────────────────────────────────────────────────────────

/**
 * Best-effort position that never rejects.
 *
 * `needed` is false when the company has geofencing off — the server discards
 * coordinates then, so blocking a kiosk queue on a GPS fix buys nothing.
 * `maximumAge` accepts a recent cached fix rather than forcing a cold lock.
 */
export function getGeolocation(needed: boolean): Promise<GeolocationCoordinates | null> {
  return new Promise((resolve) => {
    if (!needed || !navigator.geolocation) {
      resolve(null);
      return;
    }
    navigator.geolocation.getCurrentPosition(
      (pos) => resolve(pos.coords),
      () => resolve(null),
      { timeout: 8000, maximumAge: 60_000 }
    );
  });
}

// ─── Company-timezone clock ───────────────────────────────────────────────────

/**
 * Wall-clock parts in the company's timezone.
 *
 * Attendance days are bucketed in the company's zone, not the browser's, so a
 * device left on the wrong timezone must still see the time its check-in will
 * be recorded against. An unusable zone string falls back to the device clock
 * rather than throwing — a wrong-by-an-hour clock beats a blank card.
 */
export function zonedParts(date: Date, timeZone: string): { hour: number; minute: number; second: number } {
  try {
    const parts = new Intl.DateTimeFormat('en-GB', {
      timeZone,
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }).formatToParts(date);
    const read = (type: string) => Number(parts.find((p) => p.type === type)?.value ?? '0');
    // Intl renders midnight as "24" in some en-GB implementations.
    return { hour: read('hour') % 24, minute: read('minute'), second: read('second') };
  } catch {
    return { hour: date.getHours(), minute: date.getMinutes(), second: date.getSeconds() };
  }
}

const pad = (n: number) => String(n).padStart(2, '0');

/** "09:41:07" in the company's timezone — for the live clock. */
export function formatZonedClock(date: Date, timeZone: string): string {
  const { hour, minute, second } = zonedParts(date, timeZone);
  return `${pad(hour)}:${pad(minute)}:${pad(second)}`;
}

/** "9:41 AM" in the company's timezone — for recorded times. */
export function formatZonedTime(iso: string | null | undefined, timeZone: string): string {
  if (!iso) return '—';
  try {
    return new Date(iso).toLocaleTimeString('en-MY', {
      timeZone,
      hour: 'numeric',
      minute: '2-digit',
    });
  } catch {
    return new Date(iso).toLocaleTimeString('en-MY', { hour: 'numeric', minute: '2-digit' });
  }
}

/** "Monday, 27 July" in the company's timezone — for the date line. */
export function formatZonedDateLine(date: Date, timeZone: string): string {
  try {
    return date.toLocaleDateString('en-MY', {
      timeZone,
      weekday: 'long',
      day: 'numeric',
      month: 'long',
    });
  } catch {
    return date.toLocaleDateString('en-MY', { weekday: 'long', day: 'numeric', month: 'long' });
  }
}

// ─── Durations ────────────────────────────────────────────────────────────────

/** "3h 12m" / "48m" / "just now" — the elapsed-session readout. */
export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 60_000) return 'just now';
  const totalMinutes = Math.floor(ms / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return hours > 0 ? `${hours}h ${pad(minutes)}m` : `${minutes}m`;
}

// ─── Shift schedule ───────────────────────────────────────────────────────────

/** "09:00:00" or "09:00" → minutes since midnight; null if unparseable. */
export function parseScheduleTime(hms: string | null | undefined): number | null {
  if (!hms) return null;
  const match = /^(\d{1,2}):(\d{2})/.exec(hms.trim());
  if (!match) return null;
  const hour = Number(match[1]);
  const minute = Number(match[2]);
  if (hour > 23 || minute > 59) return null;
  return hour * 60 + minute;
}

/** "9:00 am" from a "HH:MM:SS" schedule time. */
export function formatScheduleTime(hms: string | null | undefined): string | null {
  const minutes = parseScheduleTime(hms);
  if (minutes === null) return null;
  // Formatted through the same locale path as `formatZonedTime` so a shift
  // start and a recorded check-in never disagree on casing in the same card.
  // The date is irrelevant — a schedule is a wall-clock time — so any date
  // carrying those local hours reads back correctly.
  return new Date(2000, 0, 1, Math.floor(minutes / 60), minutes % 60)
    .toLocaleTimeString('en-MY', { hour: 'numeric', minute: '2-digit' });
}

export type ShiftStanding =
  | { kind: 'unknown' }
  | { kind: 'early'; minutes: number }
  | { kind: 'grace'; minutes: number }
  | { kind: 'late'; minutes: number };

/**
 * Where the current moment sits relative to the shift start.
 *
 * This mirrors the server's rule — late iff now > start + grace — so the card
 * can warn *before* the employee taps rather than reporting "late" afterwards.
 * It is advisory only: the backend re-derives status from the DB clock and its
 * own schedule lookup, which may be a per-employee row this endpoint cannot see.
 */
export function shiftStanding(
  nowMinutes: number,
  startMinutes: number | null,
  graceMinutes: number
): ShiftStanding {
  if (startMinutes === null) return { kind: 'unknown' };
  const cutoff = startMinutes + Math.max(0, graceMinutes);
  if (nowMinutes < startMinutes) return { kind: 'early', minutes: startMinutes - nowMinutes };
  if (nowMinutes <= cutoff) return { kind: 'grace', minutes: nowMinutes - startMinutes };
  return { kind: 'late', minutes: nowMinutes - cutoff };
}

/** Short haptic confirmation. Progressive enhancement — absent on iOS Safari. */
export function buzz(pattern: number | number[]): void {
  try {
    navigator.vibrate?.(pattern);
  } catch {
    /* vibration is never the sole confirmation — ignore refusals */
  }
}
