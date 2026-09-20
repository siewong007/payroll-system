import { useCallback, useEffect, useState } from 'react';
import { useSearchParams, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { CheckCircle2, XCircle, Loader2, MapPin, QrCode, Info, RefreshCw, Navigation } from 'lucide-react';
import { checkInQr, getAttendanceMethod } from '@/api/attendance';
import { useAuth } from '@/context/AuthContext';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { classifyCheckInError, getGeolocation } from '@/lib/attendance';
import { getErrorMessage, translateServerMessage } from '@/lib/utils';
import { formatTimeSeconds } from '@/lib/format';

type State =
  | 'locating'
  | 'checking-in'
  | 'success'
  | 'already-checked-in'
  | 'location-required'
  | 'outside-geofence'
  | 'error'
  | 'no-token'
  | 'login-required';

export function AttendanceScanPage() {
  const { t } = useTranslation();
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
    let needsLocation: boolean;
    try {
      const method = await getAttendanceMethod();
      needsLocation = method.geofence_mode !== 'none';
    } catch {
      // Bootstrap unavailable — try for a fix, the server decides anyway.
      needsLocation = true;
    }

    setState(needsLocation ? 'locating' : 'checking-in');
    const coords = await getGeolocation(needsLocation);

    setState('checking-in');
    try {
      const result = await checkInQr(token, coords?.latitude, coords?.longitude);
      setRecord(result);
      setState('success');
    } catch (e: unknown) {
      const err = e as { response?: { status?: number; data?: { error?: string } } };
      const raw = err.response?.data?.error;
      const message = getErrorMessage(e, t('attendance.scan.checkInFailed'));
      setError(message);

      // Branch on what actually went wrong. Every failure — including "you
      // already checked in", which means attendance *was* recorded — used to
      // show the same dead-end red card. Note that an enforce-mode rejection
      // reads "…check in from an approved office location", so matching a bare
      // /location/ sent an off-site employee to a "grant permission and retry"
      // screen that could never succeed; `classifyCheckInError` splits the two.
      switch (classifyCheckInError(raw ?? message)) {
        case 'already-checked-in': setState('already-checked-in'); break;
        case 'outside-geofence':   setState('outside-geofence'); break;
        case 'location-permission': setState('location-required'); break;
        default:                   setState('error');
      }
    }
  }, [token, t]); // eslint-disable-line react-hooks/exhaustive-deps -- Babel cannot identify setError as a stable React setter.

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
      setError(t('attendance.scan.noEmployeeProfile'));
      return;
    }

    void runCheckIn();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- t must not re-arm a check-in attempt on language switch; the visible text is re-rendered anyway via state
  }, [token, authLoading, isAuthenticated, navigate, user, runCheckIn, attempt]);

  const formatTime = (iso: string) => formatTimeSeconds(iso);

  const retry = () => setAttempt((n) => n + 1);

  return (
    <div className="theme-portal min-h-screen bg-[var(--background)] flex items-center justify-center p-6">
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
                <h2 className="text-xl font-bold text-gray-900 mb-1">{t('attendance.scan.locating')}</h2>
                <p className="text-sm text-gray-500">{t('attendance.scan.locatingBody')}</p>
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
                <h2 className="text-xl font-bold text-gray-900 mb-1">{t('attendance.scan.checkingIn')}</h2>
                <p className="text-sm text-gray-500">{t('attendance.scan.checkingInBody')}</p>
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
                <h2 className="text-xl font-bold text-gray-900 mb-1">{t('attendance.scan.checkedIn')}</h2>
                {record && (
                  <p className="text-sm text-gray-500">
                    {t('attendance.scan.welcome', { name: user?.full_name })}
                    <br />
                    {t('attendance.scan.atTime', { time: formatTime(record.check_in_at) })}
                  </p>
                )}
              </div>
              <div className="w-full bg-emerald-50 rounded-2xl p-4 text-emerald-700 text-sm">
                {t('attendance.scan.recorded')}
              </div>
              <button
                onClick={() => navigate('/portal/attendance')}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors"
              >
                {t('attendance.scan.viewMyAttendance')}
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
                <h2 className="text-xl font-bold text-gray-900 mb-1">{t('attendance.scan.alreadyTitle')}</h2>
                <p className="text-sm text-gray-500">
                  {t('attendance.scan.alreadyBody')}
                </p>
              </div>
              <button
                onClick={() => navigate('/portal/attendance')}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors"
              >
                {t('attendance.scan.viewMyAttendance')}
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
                <h2 className="text-xl font-bold text-gray-900 mb-1">{t('attendance.scan.locationNeeded')}</h2>
                <p className="text-sm text-gray-500">{translateServerMessage(error)}</p>
              </div>
              <button
                onClick={retry}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors flex items-center justify-center gap-2"
              >
                <RefreshCw className="w-4 h-4" /> {t('attendance.scan.enableLocationRetry')}
              </button>
              <button
                onClick={() => navigate('/portal/attendance')}
                className="w-full py-3 border border-gray-200 text-gray-700 rounded-2xl text-sm font-semibold hover:bg-gray-50 transition-colors"
              >
                {t('attendance.scan.goToPortal')}
              </button>
            </>
          )}

          {/* Off-site — the position was resolved fine, it just isn't an
              approved one. Retrying from here is only useful after moving, so
              the copy says so instead of blaming permissions. */}
          {state === 'outside-geofence' && (
            <>
              <div className="w-20 h-20 bg-amber-50 rounded-full flex items-center justify-center">
                <Navigation className="w-10 h-10 text-amber-500" />
              </div>
              <div>
                <h2 className="text-xl font-bold text-gray-900 mb-1">{t('attendance.scan.notAtLocation')}</h2>
                <p className="text-sm text-gray-500">{translateServerMessage(error)}</p>
              </div>
              <button
                onClick={retry}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors flex items-center justify-center gap-2"
              >
                <RefreshCw className="w-4 h-4" /> {t('attendance.scan.movedRetry')}
              </button>
              <button
                onClick={() => navigate('/portal/attendance')}
                className="w-full py-3 border border-gray-200 text-gray-700 rounded-2xl text-sm font-semibold hover:bg-gray-50 transition-colors"
              >
                {t('attendance.scan.goToPortal')}
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
                <h2 className="text-xl font-bold text-gray-900 mb-1">{t('attendance.scan.failedTitle')}</h2>
                <p className="text-sm text-gray-500">{translateServerMessage(error)}</p>
              </div>
              <button
                onClick={retry}
                className="w-full py-3 bg-teal-700 text-white rounded-2xl text-sm font-semibold hover:bg-teal-800 transition-colors flex items-center justify-center gap-2"
              >
                <RefreshCw className="w-4 h-4" /> {t('common.retry')}
              </button>
              <button
                onClick={() => navigate('/portal/attendance')}
                className="w-full py-3 border border-gray-200 text-gray-700 rounded-2xl text-sm font-semibold hover:bg-gray-50 transition-colors"
              >
                {t('attendance.scan.goToPortal')}
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
                <h2 className="text-xl font-bold text-gray-900 mb-1">{t('attendance.scan.invalidQr')}</h2>
                <p className="text-sm text-gray-500">{t('attendance.scan.invalidQrBody')}</p>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
