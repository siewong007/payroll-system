import { useId, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { QrCode, Fingerprint, ShieldCheck, CheckCircle2, AlertCircle, Info, Building2, Loader2 } from 'lucide-react';
import {
  getPlatformAttendanceMethod, setPlatformAttendanceMethod, type AttendanceMethod,
} from '@/api/attendance';
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { StepUpConfirmModal } from '@/components/ui/StepUpConfirmModal';
import { useAuth } from '@/context/AuthContext';
import { userCan } from '@/lib/usePermissions';
import { getErrorMessage } from '@/lib/utils';
import { Navigate } from 'react-router';
import type { StepUpCredentials } from '@/types';

export function AttendanceSettings() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [stepUpError, setStepUpError] = useState('');
  const overrideHeadingId = useId();
  const canManagePlatform = userCan(user, 'manage_platform_settings');

  const { data, isLoading, isError, error: loadError, refetch } = useQuery({
    queryKey: ['platform-attendance-method'],
    queryFn: getPlatformAttendanceMethod,
    enabled: canManagePlatform,
  });

  const [draftMethod, setDraftMethod] = useState<AttendanceMethod | null>(null);
  const [draftOverride, setDraftOverride] = useState<boolean | null>(null);

  const method: AttendanceMethod =
    draftMethod ?? ((data?.method as AttendanceMethod | undefined) ?? 'qr_code');
  const allowOverride = draftOverride ?? (data?.allow_company_override ?? false);

  const mutation = useMutation({
    mutationFn: (stepUp: StepUpCredentials) =>
      setPlatformAttendanceMethod(method, allowOverride, stepUp),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['platform-attendance-method'] });
      void queryClient.invalidateQueries({ queryKey: ['attendance-method'] });
      setDraftMethod(null);
      setDraftOverride(null);
      setConfirmOpen(false);
      setStepUpError('');
      setSaved(true);
      setError('');
      setTimeout(() => setSaved(false), 3000);
    },
    onError: (err) => {
      const msg = getErrorMessage(err, t('attendanceSettings.saveFailed'));
      setStepUpError(msg);
      setError(msg);
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
            <Skeleton className="h-36 w-full rounded-xl" />
            <Skeleton className="h-36 w-full rounded-xl" />
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
        <div className="bg-[var(--card)] border border-[var(--border)] rounded-xl divide-y divide-[var(--border)]">
          <div className="p-6">
            <h2 className="font-semibold text-[var(--foreground)] mb-1">{t('attendanceSettings.methodTitle')}</h2>
            <p className="text-sm text-[var(--muted-foreground)] mb-5">
              {t('attendanceSettings.methodDescription')}
            </p>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              {/* QR Code Option */}
              <button
                type="button"
                aria-pressed={method === 'qr_code'}
                onClick={() => { setDraftMethod('qr_code'); setSaved(false); }}
                className={`flex flex-col gap-3 p-4 rounded-xl border text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors ${
                  method === 'qr_code'
                    ? 'border-[var(--accent-1)] bg-[var(--accent-soft)]'
                    : 'border-[var(--border)] hover:bg-[var(--surface-subtle)] bg-[var(--surface-subtle)]/50'
                }`}
              >
                <div className={`w-10 h-10 rounded-lg flex items-center justify-center ${
                  method === 'qr_code' ? 'bg-[var(--accent-1)] text-white' : 'bg-[var(--surface-subtle)] border border-[var(--border)] text-[var(--muted-foreground)]'
                }`}>
                  <QrCode className="w-5 h-5" aria-hidden="true" />
                </div>
                <div>
                  <p className="font-semibold text-[var(--foreground)]">{t('attendanceSettings.qrTitle')}</p>
                  <p className="text-xs text-[var(--muted-foreground)] mt-0.5">
                    {t('attendanceSettings.qrDescription')}
                  </p>
                </div>
                {method === 'qr_code' && (
                  <div className="flex items-center gap-1.5 text-[var(--accent-1)] text-xs font-medium">
                    <CheckCircle2 className="w-3.5 h-3.5" aria-hidden="true" /> {t('common.selectedState')}
                  </div>
                )}
              </button>

              {/* Face ID Option */}
              <button
                type="button"
                aria-pressed={method === 'face_id'}
                onClick={() => { setDraftMethod('face_id'); setSaved(false); }}
                className={`flex flex-col gap-3 p-4 rounded-xl border text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors ${
                  method === 'face_id'
                    ? 'border-[var(--accent-1)] bg-[var(--accent-soft)]'
                    : 'border-[var(--border)] hover:bg-[var(--surface-subtle)] bg-[var(--surface-subtle)]/50'
                }`}
              >
                <div className={`w-10 h-10 rounded-lg flex items-center justify-center ${
                  method === 'face_id' ? 'bg-[var(--accent-1)] text-white' : 'bg-[var(--surface-subtle)] border border-[var(--border)] text-[var(--muted-foreground)]'
                }`}>
                  <Fingerprint className="w-5 h-5" aria-hidden="true" />
                </div>
                <div>
                  <p className="font-semibold text-[var(--foreground)]">{t('attendanceSettings.faceIdTitle')}</p>
                  <p className="text-xs text-[var(--muted-foreground)] mt-0.5">
                    {t('attendanceSettings.faceIdDescription')}
                  </p>
                </div>
                {method === 'face_id' && (
                  <div className="flex items-center gap-1.5 text-[var(--accent-1)] text-xs font-medium">
                    <CheckCircle2 className="w-3.5 h-3.5" aria-hidden="true" /> {t('common.selectedState')}
                  </div>
                )}
              </button>

              {/* Combined QR + Face ID Option */}
              <button
                type="button"
                aria-pressed={method === 'qr_and_face_id'}
                onClick={() => { setDraftMethod('qr_and_face_id'); setSaved(false); }}
                className={`flex flex-col gap-3 p-4 rounded-xl border text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors ${
                  method === 'qr_and_face_id'
                    ? 'border-[var(--accent-1)] bg-[var(--accent-soft)]'
                    : 'border-[var(--border)] hover:bg-[var(--surface-subtle)] bg-[var(--surface-subtle)]/50'
                }`}
              >
                <div className={`w-10 h-10 rounded-lg flex items-center justify-center ${
                  method === 'qr_and_face_id' ? 'bg-[var(--accent-1)] text-white' : 'bg-[var(--surface-subtle)] border border-[var(--border)] text-[var(--muted-foreground)]'
                }`}>
                  <ShieldCheck className="w-5 h-5" aria-hidden="true" />
                </div>
                <div>
                  <p className="font-semibold text-[var(--foreground)]">{t('attendanceSettings.qrFaceIdTitle')}</p>
                  <p className="text-xs text-[var(--muted-foreground)] mt-0.5">
                    {t('attendanceSettings.qrFaceIdDescription')}
                  </p>
                </div>
                {method === 'qr_and_face_id' && (
                  <div className="flex items-center gap-1.5 text-[var(--accent-1)] text-xs font-medium">
                    <CheckCircle2 className="w-3.5 h-3.5" aria-hidden="true" /> {t('common.selectedState')}
                  </div>
                )}
              </button>
            </div>

            {/* Face ID note */}
            {(method === 'face_id' || method === 'qr_and_face_id') && (
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
          <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 px-6 py-4 bg-[var(--surface-subtle)] rounded-b-xl">
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
              onClick={() => {
                setStepUpError('');
                setError('');
                setConfirmOpen(true);
              }}
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
        <div className="bg-amber-50 dark:bg-amber-500/10 border border-amber-200 dark:border-amber-500/25 rounded-xl p-4 text-sm text-amber-800 dark:text-amber-200">
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

      <StepUpConfirmModal
        open={confirmOpen}
        onClose={() => {
          if (!mutation.isPending) {
            setConfirmOpen(false);
            setStepUpError('');
          }
        }}
        title={t('attendanceSettings.confirmSaveTitle')}
        description={t('attendanceSettings.confirmSaveDescription')}
        confirmLabel={t('common.saveChanges')}
        pendingLabel={t('common.saving')}
        pending={mutation.isPending}
        error={stepUpError}
        onConfirm={(creds) => {
          setStepUpError('');
          mutation.mutate(creds);
        }}
      />
    </div>
  );
}
