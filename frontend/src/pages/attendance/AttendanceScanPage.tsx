import { useCallback, useEffect, useState } from 'react';
import { useSearchParams, useNavigate } from 'react-router-dom';
import { CheckCircle2, XCircle, Loader2, MapPin, QrCode, Info, RefreshCw } from 'lucide-react';
import { checkInQr, getAttendanceMethod } from '@/api/attendance';
import { useAuth } from '@/context/AuthContext';
import { BrandLogo } from '@/components/ui/BrandLogo';

type State =
  | 'locating'
  | 'checking-in'
  | 'success'
  | 'already-checked-in'
  | 'location-required'
  | 'error'
  | 'no-token'
  | 'login-required';

/**
 * Resolve a position without ever rejecting. `needed` is false when the
 * company has geofencing off — the server discards coordinates then, so
 * blocking the queue on a GPS fix buys nothing. A recent cached fix is
 * accepted rather than forcing a cold lock.
 */
function getCoords(needed: boolean): Promise<GeolocationCoordinates | null> {
  return new Promise((resolve) => {
    if (!needed || !navigator.geolocation) { resolve(null); return; }
    navigator.geolocation.getCurrentPosition(
      (pos) => resolve(pos.coords),
      () => resolve(null),
      { timeout: 8000, maximumAge: 60_000 }
    );
  });
}

export function AttendanceScanPage() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const { isAuthenticated, isLoading: authLoading, user } = useAuth();
  const token = searchParams.get('token');

  const [state, setState] = useState<State>('locating');
  const [error, setError] = useState('');
  const [record, setRecord] = useState<{ check_in_at: string } | null>(null);
  const [attempt, setAttempt] = useState(0);

  const runCheckIn = useCallback(async () => {
    if (!token) return;

    // Ask the server whether coordinates matter before paying for a fix.
    let needsLocation = false;
    try {
      const method = await getAttendanceMethod();
      needsLocation = method.geofence_mode !== 'none';
    } catch {
      // Bootstrap unavailable — try for a fix, the server decides anyway.
      needsLocation = true;
    }

    setState(needsLocation ? 'locating' : 'checking-in');
    const coords = await getCoords(needsLocation);

    setState('checking-in');
    try {
      const result = await checkInQr(token, coords?.latitude, coords?.longitude);
      setRecord(result);
      setState('success');
    } catch (e: unknown) {
      const err = e as { response?: { status?: number; data?: { error?: string } } };
      const message = err.response?.data?.error || 'Check-in failed. Please try again.';
      setError(message);

      // Branch on what actually went wrong. Previously every failure — including
      // "you already checked in", which means attendance *was* recorded — showed
      // the same dead-end red card.
      if (/already checked in/i.test(message)) {
        setState('already-checked-in');
      } else if (/location/i.test(message)) {
        setState('location-required');
      } else {
        setState('error');
      }
    }
  }, [token]); // eslint-disable-line react-hooks/exhaustive-deps -- Babel cannot identify setError as a stable React setter.

  useEffect(() => {
    if (!token) {
      setState('no-token');
      return;
    }
    if (authLoading) return;

    if (!isAuthenticated) {
      // Redirect to login, come back here after
      navigate(`/login?redirect=${encodeURIComponent(window.location.pathname + window.location.search)}`, { replace: true });
      return;
    }

    if (!user?.employee_id) {
      setState('error');
      setError('Your account is not linked to an employee profile. Please contact HR.');
      return;
    }

    void runCheckIn();
  }, [token, authLoading, isAuthenticated, navigate, user, runCheckIn, attempt]);

  const formatTime = (iso: string) =>
    new Date(iso).toLocaleTimeString('en-MY', { hour: '2-digit', minute: '2-digit', second: '2-digit' });

  const retry = () => setAttempt((n) => n + 1);

  return (
    <div className="min-h-screen bg-gray-50 flex items-center justify-center p-6">
      <div className="w-full max-w-sm">

        {/* Logo */}
        <BrandLogo variant="lockup-dark" className="h-10 w-auto mx-auto mb-8" />

        {/* Card */}
        <div className="bg-white rounded-3xl shadow-lg p-8 flex flex-col items-center text-center gap-5">

          {/* Locating */}
          {state === 'locating' && (
            <>
              <div className="w-20 h-20 bg-sky-50 rounded-full flex items-center justify-center">
                <MapPin className="w-9 h-9 text-sky-500 animate-pulse" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 mb-1">Getting Location…</h2>
                <p className="text-sm text-gray-500">Please allow location access for attendance</p>
              </div>
            </>
          )}

          {/* Checking in */}
          {state === 'checking-in' && (
            <>
              <div className="w-20 h-20 bg-violet-50 rounded-full flex items-center justify-center">
                <Loader2 className="w-9 h-9 text-violet-500 animate-spin" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 mb-1">Checking In…</h2>
                <p className="text-sm text-gray-500">Recording your attendance</p>
              </div>
            </>
          )}

          {/* Success */}
          {state === 'success' && (
            <>
              <div className="w-20 h-20 bg-emerald-50 rounded-full flex items-center justify-center">
                <CheckCircle2 className="w-10 h-10 text-emerald-500" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 mb-1">Checked In! ✓</h2>
                {record && (
                  <p className="text-sm text-gray-500">
                    Welcome, <strong className="text-gray-700">{user?.full_name}</strong>
                    <br />
                    at {formatTime(record.check_in_at)}
                  </p>
                )}
              </div>
              <div className="w-full bg-emerald-50 rounded-2xl p-4 text-emerald-700 text-sm">
                Your attendance has been recorded successfully.
              </div>
              <button
                onClick={() => navigate('/portal/attendance')}
                className="w-full py-3 bg-black text-white rounded-2xl text-sm font-semibold hover:bg-gray-800 transition-colors"
              >
                View My Attendance
              </button>
            </>
          )}

          {/* Already checked in — not a failure: attendance is recorded. */}
          {state === 'already-checked-in' && (
            <>
              <div className="w-20 h-20 bg-sky-50 rounded-full flex items-center justify-center">
                <Info className="w-10 h-10 text-sky-500" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 mb-1">You're already checked in</h2>
                <p className="text-sm text-gray-500">
                  No need to scan again — your attendance for today is recorded.
                </p>
              </div>
              <button
                onClick={() => navigate('/portal/attendance')}
                className="w-full py-3 bg-black text-white rounded-2xl text-sm font-semibold hover:bg-gray-800 transition-colors"
              >
                View My Attendance
              </button>
            </>
          )}

          {/* Location required — actionable, with a retry on the same token. */}
          {state === 'location-required' && (
            <>
              <div className="w-20 h-20 bg-amber-50 rounded-full flex items-center justify-center">
                <MapPin className="w-10 h-10 text-amber-500" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 mb-1">Location Needed</h2>
                <p className="text-sm text-gray-500">{error}</p>
              </div>
              <button
                onClick={retry}
                className="w-full py-3 bg-black text-white rounded-2xl text-sm font-semibold hover:bg-gray-800 transition-colors flex items-center justify-center gap-2"
              >
                <RefreshCw className="w-4 h-4" /> Enable location and retry
              </button>
              <button
                onClick={() => navigate('/portal/attendance')}
                className="w-full py-3 border border-gray-200 text-gray-700 rounded-2xl text-sm font-semibold hover:bg-gray-50 transition-colors"
              >
                Go to Portal
              </button>
            </>
          )}

          {/* Error */}
          {state === 'error' && (
            <>
              <div className="w-20 h-20 bg-red-50 rounded-full flex items-center justify-center">
                <XCircle className="w-10 h-10 text-red-500" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 mb-1">Check-in Failed</h2>
                <p className="text-sm text-gray-500">{error}</p>
              </div>
              <button
                onClick={retry}
                className="w-full py-3 bg-black text-white rounded-2xl text-sm font-semibold hover:bg-gray-800 transition-colors flex items-center justify-center gap-2"
              >
                <RefreshCw className="w-4 h-4" /> Try again
              </button>
              <button
                onClick={() => navigate('/portal/attendance')}
                className="w-full py-3 border border-gray-200 text-gray-700 rounded-2xl text-sm font-semibold hover:bg-gray-50 transition-colors"
              >
                Go to Portal
              </button>
            </>
          )}

          {/* No token */}
          {state === 'no-token' && (
            <>
              <div className="w-20 h-20 bg-gray-100 rounded-full flex items-center justify-center">
                <QrCode className="w-9 h-9 text-gray-400" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 mb-1">Invalid QR Code</h2>
                <p className="text-sm text-gray-500">Scan the QR code displayed at the attendance kiosk.</p>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
