import { useId, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { QrCode, Fingerprint, CheckCircle2, AlertCircle, Info, Building2, Loader2 } from 'lucide-react';
import { getPlatformAttendanceMethod, setPlatformAttendanceMethod } from '@/api/attendance';
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { useAuth } from '@/context/AuthContext';
import { userCan } from '@/lib/usePermissions';
import { getErrorMessage } from '@/lib/utils';
import { Navigate } from 'react-router';

export function AttendanceSettings() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');
  const overrideHeadingId = useId();
  const canManagePlatform = userCan(user, 'manage_platform_settings');

  const { data, isLoading, isError, error: loadError, refetch } = useQuery({
    queryKey: ['platform-attendance-method'],
    queryFn: getPlatformAttendanceMethod,
    enabled: canManagePlatform,
  });

  const [draftMethod, setDraftMethod] = useState<'qr_code' | 'face_id' | null>(null);
  const [draftOverride, setDraftOverride] = useState<boolean | null>(null);

  const method = draftMethod ?? ((data?.method as 'qr_code' | 'face_id' | undefined) ?? 'qr_code');
  const allowOverride = draftOverride ?? (data?.allow_company_override ?? false);

  const mutation = useMutation({
    mutationFn: () => setPlatformAttendanceMethod(method, allowOverride),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['platform-attendance-method'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-method'] });
      setDraftMethod(null);
      setDraftOverride(null);
      setSaved(true);
      setError('');
      setTimeout(() => setSaved(false), 3000);
    },
    onError: (err) => {
      setError(getErrorMessage(err, t('attendanceSettings.saveFailed')));
    },
  });

  if (!canManagePlatform) {
    return <Navigate to="/companies" replace />;
  }

  if (isLoading) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('attendanceSettings.title')} subtitle={t('attendanceSettings.subtitle')} />
        <div role="status" aria-label={t('common.loading')} className="max-w-2xl card space-y-5">
          <Skeleton className="h-5 w-48" />
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <Skeleton className="h-36 w-full rounded-2xl" />
            <Skeleton className="h-36 w-full rounded-2xl" />
          </div>
          <Skeleton className="h-14 w-full rounded-xl" />
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('attendanceSettings.title')} subtitle={t('attendanceSettings.subtitle')} />
        <div className="max-w-2xl card">
          <EmptyState
            icon={AlertCircle}
            title={t('common.loadFailed')}
            description={getErrorMessage(loadError, t('common.error'))}
            action={
              <button type="button" onClick={() => void refetch()} className="btn-secondary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader title={t('attendanceSettings.title')} subtitle={t('attendanceSettings.subtitle')} />

      <div className="max-w-2xl space-y-6">

        {/* Method Selection */}
        <div className="bg-white dark:bg-slate-900 border border-gray-200/80 dark:border-slate-800 rounded-2xl shadow-sm divide-y divide-gray-100 dark:divide-slate-800">
          <div className="p-6">
            <h2 className="font-semibold text-gray-900 dark:text-slate-100 mb-1">{t('attendanceSettings.methodTitle')}</h2>
            <p className="text-sm text-gray-600 dark:text-slate-300 mb-5">
              {t('attendanceSettings.methodDescription')}
            </p>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              {/* QR Code Option */}
              <button
                type="button"
                aria-pressed={method === 'qr_code'}
                onClick={() => { setDraftMethod('qr_code'); setSaved(false); }}
                className={`flex flex-col gap-4 p-5 rounded-2xl border-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-all ${
                  method === 'qr_code'
                    ? 'border-violet-500 bg-violet-50 dark:bg-violet-500/10'
                    : 'border-gray-200 dark:border-slate-700 hover:border-gray-300 dark:hover:border-slate-600 bg-gray-50 dark:bg-slate-800/60'
                }`}
              >
                <div className={`w-12 h-12 rounded-xl flex items-center justify-center ${
                  method === 'qr_code' ? 'bg-violet-100 dark:bg-violet-500/20' : 'bg-gray-100 dark:bg-slate-800'
                }`}>
                  <QrCode className={`w-6 h-6 ${method === 'qr_code' ? 'text-violet-600 dark:text-violet-300' : 'text-gray-500 dark:text-slate-400'}`} aria-hidden="true" />
                </div>
                <div>
                  <p className="font-semibold text-gray-900 dark:text-slate-100">{t('attendanceSettings.qrTitle')}</p>
                  <p className="text-xs text-gray-600 dark:text-slate-300 mt-0.5">
                    {t('attendanceSettings.qrDescription')}
                  </p>
                </div>
                {method === 'qr_code' && (
                  <div className="flex items-center gap-1.5 text-violet-700 dark:text-violet-300 text-xs font-medium">
                    <CheckCircle2 className="w-3.5 h-3.5" aria-hidden="true" /> {t('common.selectedState')}
                  </div>
                )}
              </button>

              {/* Face ID Option */}
              <button
                type="button"
                aria-pressed={method === 'face_id'}
                onClick={() => { setDraftMethod('face_id'); setSaved(false); }}
                className={`flex flex-col gap-4 p-5 rounded-2xl border-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-all ${
                  method === 'face_id'
                    ? 'border-sky-500 bg-sky-50 dark:bg-sky-500/10'
                    : 'border-gray-200 dark:border-slate-700 hover:border-gray-300 dark:hover:border-slate-600 bg-gray-50 dark:bg-slate-800/60'
                }`}
              >
                <div className={`w-12 h-12 rounded-xl flex items-center justify-center ${
                  method === 'face_id' ? 'bg-sky-100 dark:bg-sky-500/20' : 'bg-gray-100 dark:bg-slate-800'
                }`}>
                  <Fingerprint className={`w-6 h-6 ${method === 'face_id' ? 'text-sky-600 dark:text-sky-300' : 'text-gray-500 dark:text-slate-400'}`} aria-hidden="true" />
                </div>
                <div>
                  <p className="font-semibold text-gray-900 dark:text-slate-100">{t('attendanceSettings.faceIdTitle')}</p>
                  <p className="text-xs text-gray-600 dark:text-slate-300 mt-0.5">
                    {t('attendanceSettings.faceIdDescription')}
                  </p>
                </div>
                {method === 'face_id' && (
                  <div className="flex items-center gap-1.5 text-sky-700 dark:text-sky-300 text-xs font-medium">
                    <CheckCircle2 className="w-3.5 h-3.5" aria-hidden="true" /> {t('common.selectedState')}
                  </div>
                )}
              </button>
            </div>

            {/* Face ID note */}
            {method === 'face_id' && (
              <div className="mt-4 flex items-start gap-2.5 bg-sky-50 dark:bg-sky-500/10 border border-sky-100 dark:border-sky-500/20 rounded-xl p-4 text-sm text-sky-800 dark:text-sky-200">
                <Info className="w-4 h-4 shrink-0 mt-0.5" aria-hidden="true" />
                <span>
                  <Trans i18nKey="attendanceSettings.faceIdNote" components={{ b: <strong /> }} />
                </span>
              </div>
            )}
          </div>

          {/* Override Setting */}
          <div className="p-6">
            <div className="flex items-start justify-between gap-4">
              <div className="flex items-start gap-3">
                <div className="mt-0.5">
                  <Building2 className="w-5 h-5 text-gray-500 dark:text-slate-400" aria-hidden="true" />
                </div>
                <div>
                  <p id={overrideHeadingId} className="font-medium text-gray-900 dark:text-slate-100">
                    {t('attendanceSettings.overrideTitle')}
                  </p>
                  <p className="text-sm text-gray-600 dark:text-slate-300 mt-0.5">
                    {t('attendanceSettings.overrideDescription')}
                  </p>
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={allowOverride}
                aria-labelledby={overrideHeadingId}
                onClick={() => { setDraftOverride(!allowOverride); setSaved(false); }}
                className={`relative inline-flex h-7 w-12 shrink-0 items-center rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors ${
                  allowOverride ? 'bg-gray-900 dark:bg-[var(--accent-1)]' : 'bg-gray-300 dark:bg-slate-700'
                }`}
              >
                <span className={`inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform ${
                  allowOverride ? 'translate-x-6' : 'translate-x-1'
                }`} />
              </button>
            </div>
          </div>

          {/* Save Bar */}
          <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 px-6 py-4 bg-gray-50 dark:bg-slate-950/60 rounded-b-2xl">
            <div className="min-h-5">
              {saved && (
                <span role="status" className="flex items-center gap-1.5 text-sm text-emerald-700 dark:text-emerald-400 font-medium">
                  <CheckCircle2 className="w-4 h-4" aria-hidden="true" /> {t('attendanceSettings.saved')}
                </span>
              )}
              {error && (
                <span role="alert" className="flex items-center gap-1.5 text-sm text-red-700 dark:text-red-300">
                  <AlertCircle className="w-4 h-4" aria-hidden="true" /> {error}
                </span>
              )}
            </div>
            <button
              type="button"
              onClick={() => mutation.mutate()}
              disabled={mutation.isPending}
              className="btn-primary !rounded-xl shrink-0 w-full sm:w-auto inline-flex items-center justify-center gap-2"
            >
              {mutation.isPending ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                  {t('common.saving')}
                </>
              ) : (
                t('common.saveChanges')
              )}
            </button>
          </div>
        </div>

        {/* Info Box */}
        <div className="bg-amber-50 dark:bg-amber-500/10 border border-amber-100 dark:border-amber-500/20 rounded-2xl p-5 text-sm text-amber-800 dark:text-amber-200">
          <div className="flex items-start gap-2.5">
            <Info className="w-4 h-4 shrink-0 mt-0.5 text-amber-600 dark:text-amber-400" />
            <div>
              <p className="font-semibold mb-1">{t('attendanceSettings.howItWorks')}</p>
              <ul className="space-y-1 text-amber-700 dark:text-amber-200/90">
                <li>• <Trans i18nKey="attendanceSettings.note1" components={{ b: <strong /> }} /></li>
                <li>• <Trans i18nKey="attendanceSettings.note2" components={{ b: <strong /> }} /></li>
                <li>• <Trans i18nKey="attendanceSettings.note3" components={{ b: <strong /> }} /></li>
                <li>• <Trans i18nKey="attendanceSettings.note4" components={{ b: <strong /> }} /></li>
              </ul>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
