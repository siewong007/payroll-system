import { useEffect, useState } from 'react';
import { useSearchParams, useNavigate } from 'react-router-dom';
import { CheckCircle2, XCircle, Loader2, MapPin, QrCode } from 'lucide-react';
import { checkInQr } from '@/api/attendance';
import { useAuth } from '@/context/AuthContext';
import { BrandLogo } from '@/components/ui/BrandLogo';

type State = 'locating' | 'checking-in' | 'success' | 'error' | 'no-token' | 'login-required';

export function AttendanceScanPage() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const { isAuthenticated, isLoading: authLoading, user } = useAuth();
  const token = searchParams.get('token');

  const [state, setState] = useState<State>('locating');
  const [error, setError] = useState('');
  const [record, setRecord] = useState<{ check_in_at: string } | null>(null);

  useEffect(() => {
    if (!token) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
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

    // Get geolocation, then check in
    setState('locating');
    navigator.geolocation.getCurrentPosition(
      async (pos) => {
        setState('checking-in');
        try {
          const result = await checkInQr(token, pos.coords.latitude, pos.coords.longitude);
          setRecord(result);
          setState('success');
        } catch (e: unknown) {
          const err = e as { response?: { data?: { error?: string } } };
          setError(err.response?.data?.error || 'Check-in failed. Please try again.');
          setState('error');
        }
      },
      async (geoErr) => {
        // Still attempt check-in without location
        if (geoErr.code === geoErr.PERMISSION_DENIED) {
          setState('checking-in');
          try {
            const result = await checkInQr(token);
            setRecord(result);
            setState('success');
          } catch (e: unknown) {
            const err = e as { response?: { data?: { error?: string } } };
            setError(err.response?.data?.error || 'Check-in failed. Please try again.');
            setState('error');
          }
        } else {
          setState('checking-in');
          try {
            const result = await checkInQr(token);
            setRecord(result);
            setState('success');
          } catch (e: unknown) {
            const err = e as { response?: { data?: { error?: string } } };
            setError(err.response?.data?.error || 'Check-in failed. Please try again.');
            setState('error');
          }
        }
      },
      { timeout: 8000, maximumAge: 0 }
    );
  }, [token, authLoading, isAuthenticated, navigate, user]);

  const formatTime = (iso: string) =>
    new Date(iso).toLocaleTimeString('en-MY', { hour: '2-digit', minute: '2-digit', second: '2-digit' });

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
