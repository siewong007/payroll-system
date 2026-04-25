import { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { QrCode, Fingerprint, Shield, CheckCircle2, AlertCircle, Info, Building2 } from 'lucide-react';
import { getPlatformAttendanceMethod, setPlatformAttendanceMethod } from '@/api/attendance';
import { useAuth } from '@/context/AuthContext';
import { hasAnyRole } from '@/lib/roles';
import { Navigate } from 'react-router-dom';

export function AttendanceSettings() {
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');

  const { data, isLoading } = useQuery({
    queryKey: ['platform-attendance-method'],
    queryFn: getPlatformAttendanceMethod,
    enabled: hasAnyRole(user, ['super_admin']),
  });

  const [method, setMethod] = useState<'qr_code' | 'face_id'>('qr_code');
  const [allowOverride, setAllowOverride] = useState(false);
  const [initialized, setInitialized] = useState(false);

  const mutation = useMutation({
    mutationFn: () => setPlatformAttendanceMethod(method, allowOverride),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['platform-attendance-method'] });
      setSaved(true);
      setError('');
      setTimeout(() => setSaved(false), 3000);
    },
    onError: () => {
      setError('Failed to save settings. Please try again.');
    },
  });

  useEffect(() => {
    if (data && !initialized) {
      setMethod(data.method as 'qr_code' | 'face_id');
      setAllowOverride(data.allow_company_override);
      setInitialized(true);
    }
  }, [data, initialized]);

  if (!hasAnyRole(user, ['super_admin'])) {
    return <Navigate to="/companies" replace />;
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-center gap-3 mb-6">
        <div className="w-10 h-10 bg-black rounded-xl flex items-center justify-center">
          <Shield className="w-5 h-5 text-white" />
        </div>
        <div>
          <h1 className="text-2xl font-bold text-gray-900">Attendance Settings</h1>
          <p className="text-sm text-gray-500 mt-0.5">Control how all companies record attendance</p>
        </div>
      </div>

      <div className="max-w-2xl space-y-6">

        {/* Method Selection */}
        <div className="bg-white rounded-2xl shadow divide-y divide-gray-100">
          <div className="p-6">
            <h2 className="font-semibold text-gray-900 mb-1">Global Attendance Method</h2>
            <p className="text-sm text-gray-500 mb-5">
              Set the default check-in method for all companies. This applies to every company unless you allow per-company overrides.
            </p>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              {/* QR Code Option */}
              <button
                onClick={() => setMethod('qr_code')}
                className={`flex flex-col gap-4 p-5 rounded-2xl border-2 text-left transition-all ${
                  method === 'qr_code'
                    ? 'border-violet-500 bg-violet-50'
                    : 'border-gray-200 hover:border-gray-300 bg-gray-50'
                }`}
              >
                <div className={`w-12 h-12 rounded-xl flex items-center justify-center ${
                  method === 'qr_code' ? 'bg-violet-100' : 'bg-gray-100'
                }`}>
                  <QrCode className={`w-6 h-6 ${method === 'qr_code' ? 'text-violet-600' : 'text-gray-400'}`} />
                </div>
                <div>
                  <p className="font-semibold text-gray-900">QR Code</p>
                  <p className="text-xs text-gray-500 mt-0.5">
                    Admin displays a rotating QR on the kiosk. Employees scan with their phone.
                  </p>
                </div>
                {method === 'qr_code' && (
                  <div className="flex items-center gap-1.5 text-violet-600 text-xs font-medium">
                    <CheckCircle2 className="w-3.5 h-3.5" /> Selected
                  </div>
                )}
              </button>

              {/* Face ID Option */}
              <button
                onClick={() => setMethod('face_id')}
                className={`flex flex-col gap-4 p-5 rounded-2xl border-2 text-left transition-all ${
                  method === 'face_id'
                    ? 'border-sky-500 bg-sky-50'
                    : 'border-gray-200 hover:border-gray-300 bg-gray-50'
                }`}
              >
                <div className={`w-12 h-12 rounded-xl flex items-center justify-center ${
                  method === 'face_id' ? 'bg-sky-100' : 'bg-gray-100'
                }`}>
                  <Fingerprint className={`w-6 h-6 ${method === 'face_id' ? 'text-sky-600' : 'text-gray-400'}`} />
                </div>
                <div>
                  <p className="font-semibold text-gray-900">Face ID / Passkey</p>
                  <p className="text-xs text-gray-500 mt-0.5">
                    Employees use biometric (Face ID, Touch ID, Windows Hello) on their device.
                  </p>
                </div>
                {method === 'face_id' && (
                  <div className="flex items-center gap-1.5 text-sky-600 text-xs font-medium">
                    <CheckCircle2 className="w-3.5 h-3.5" /> Selected
                  </div>
                )}
              </button>
            </div>

            {/* Face ID note */}
            {method === 'face_id' && (
              <div className="mt-4 flex items-start gap-2.5 bg-sky-50 border border-sky-100 rounded-xl p-4 text-sm text-sky-700">
                <Info className="w-4 h-4 shrink-0 mt-0.5" />
                <span>
                  Employees must have a registered passkey (Face ID, Touch ID, Windows Hello) to use this method.
                  They can register one from <strong>Settings → Security Keys</strong>.
                </span>
              </div>
            )}
          </div>

          {/* Override Setting */}
          <div className="p-6">
            <div className="flex items-start justify-between gap-4">
              <div className="flex items-start gap-3">
                <div className="mt-0.5">
                  <Building2 className="w-5 h-5 text-gray-400" />
                </div>
                <div>
                  <p className="font-medium text-gray-900">Allow Company-Level Override</p>
                  <p className="text-sm text-gray-500 mt-0.5">
                    When enabled, individual company admins can choose a different method for their company.
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setAllowOverride(!allowOverride)}
                className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${
                  allowOverride ? 'bg-black' : 'bg-gray-300'
                }`}
              >
                <span className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  allowOverride ? 'translate-x-6' : 'translate-x-1'
                }`} />
              </button>
            </div>
          </div>

          {/* Save Bar */}
          <div className="flex items-center justify-between px-6 py-4 bg-gray-50 rounded-b-2xl">
            <div className="h-5">
              {saved && (
                <span className="flex items-center gap-1.5 text-sm text-emerald-600 font-medium">
                  <CheckCircle2 className="w-4 h-4" /> Settings saved
                </span>
              )}
              {error && (
                <span className="flex items-center gap-1.5 text-sm text-red-600">
                  <AlertCircle className="w-4 h-4" /> {error}
                </span>
              )}
            </div>
            <button
              onClick={() => mutation.mutate()}
              disabled={mutation.isPending}
              className="flex items-center gap-2 bg-black text-white px-5 py-2 rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50"
            >
              {mutation.isPending ? 'Saving…' : 'Save Changes'}
            </button>
          </div>
        </div>

        {/* Info Box */}
        <div className="bg-amber-50 border border-amber-100 rounded-2xl p-5 text-sm text-amber-800">
          <div className="flex items-start gap-2.5">
            <Info className="w-4 h-4 shrink-0 mt-0.5 text-amber-600" />
            <div>
              <p className="font-semibold mb-1">How this works</p>
              <ul className="space-y-1 text-amber-700">
                <li>• The global method applies to <strong>all companies</strong> immediately.</li>
                <li>• If "Allow Company Override" is on, admins can go to <strong>Attendance → Method</strong> to switch.</li>
                <li>• QR codes are valid for <strong>60 seconds</strong> and are single-use for security.</li>
                <li>• Face ID requires employees to register a passkey first.</li>
              </ul>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
