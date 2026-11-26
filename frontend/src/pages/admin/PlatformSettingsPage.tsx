import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AlertTriangle, CheckCircle2, Clock, Loader2, RefreshCw, ShieldCheck, XCircle } from 'lucide-react';
import {
  getPlatformHealth,
  getTwoFactorPolicy,
  setTwoFactorPolicy,
  type AdminJobHealth,
} from '@/api/platform';
import { EmptyState } from '@/components/ui/EmptyState';
import { FormField } from '@/components/ui/FormField';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { useAuth } from '@/context/AuthContext';
import { formatDateTime } from '@/lib/format';
import { useToast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';

function JobStatusBadge({ job }: { job: AdminJobHealth }) {
  const { t } = useTranslation();

  if (!job.last_run_at) {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-gray-100 dark:bg-slate-800 px-2.5 py-0.5 text-xs font-medium text-gray-600 dark:text-slate-300">
        <Clock className="w-3.5 h-3.5" aria-hidden="true" />
        {t('platformAdmin.health.neverRun')}
      </span>
    );
  }
  if (job.ok === false) {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-red-50 dark:bg-red-500/15 px-2.5 py-0.5 text-xs font-medium text-red-700 dark:text-red-300">
        <XCircle className="w-3.5 h-3.5" aria-hidden="true" />
        {t('platformAdmin.health.failed')}
      </span>
    );
  }
  if (job.stale) {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-amber-50 dark:bg-amber-500/15 px-2.5 py-0.5 text-xs font-medium text-amber-700 dark:text-amber-300">
        <AlertTriangle className="w-3.5 h-3.5" aria-hidden="true" />
        {t('platformAdmin.health.stale')}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-emerald-50 dark:bg-emerald-500/15 px-2.5 py-0.5 text-xs font-medium text-emerald-700 dark:text-emerald-300">
      <CheckCircle2 className="w-3.5 h-3.5" aria-hidden="true" />
      {t('platformAdmin.health.healthy')}
    </span>
  );
}

export function PlatformSettingsPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const queryClient = useQueryClient();
  const { user } = useAuth();
  const twoFactorEnabled = Boolean(user?.two_factor_enabled);

  const [enforceDraft, setEnforceDraft] = useState<boolean | null>(null);
  const [graceUntilDraft, setGraceUntilDraft] = useState<string | null>(null);
  const [actorPassword, setActorPassword] = useState('');
  const [actorTotpCode, setActorTotpCode] = useState('');
  const [policyError, setPolicyError] = useState('');

  const policyQuery = useQuery({
    queryKey: ['platform-2fa-policy'],
    queryFn: getTwoFactorPolicy,
  });

  const healthQuery = useQuery({
    queryKey: ['platform-health'],
    queryFn: getPlatformHealth,
  });

  const enforce = enforceDraft ?? policyQuery.data?.enforce ?? false;
  const graceUntil = graceUntilDraft ?? policyQuery.data?.grace_until ?? '';

  const savePolicyMutation = useMutation({
    mutationFn: () =>
      setTwoFactorPolicy({
        enforce,
        grace_until: graceUntil.trim() || null,
        actor_password: actorPassword,
        ...(twoFactorEnabled && actorTotpCode.trim()
          ? { actor_totp_code: actorTotpCode.trim() }
          : {}),
      }),
    onSuccess: (updated) => {
      queryClient.setQueryData(['platform-2fa-policy'], updated);
      setEnforceDraft(null);
      setGraceUntilDraft(null);
      setActorPassword('');
      setActorTotpCode('');
      setPolicyError('');
      toast.success(t('platformAdmin.twoFactorPolicy.saved'));
    },
    onError: (err: unknown) => {
      const msg = getErrorMessage(err, t('platformAdmin.twoFactorPolicy.saveFailed'));
      setPolicyError(msg);
      toast.error(msg);
    },
  });

  const jobRows: { key: string; label: string; health: AdminJobHealth }[] = healthQuery.data
    ? [
        {
          key: 'retention',
          label: t('platformAdmin.health.jobRetention'),
          health: healthQuery.data.jobs.retention_cleanup,
        },
        {
          key: 'auto-absent',
          label: t('platformAdmin.health.jobAutoAbsent'),
          health: healthQuery.data.jobs.auto_absent,
        },
        {
          key: 'email-outbox',
          label: t('platformAdmin.health.jobEmailOutbox'),
          health: healthQuery.data.jobs.email_outbox,
        },
        {
          key: 'push-outbox',
          label: t('platformAdmin.health.jobPushOutbox'),
          health: healthQuery.data.jobs.push_outbox,
        },
      ]
    : [];

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('platformAdmin.title')}
        subtitle={t('platformAdmin.subtitle')}
        actions={
          <button
            type="button"
            onClick={() => {
              void healthQuery.refetch();
              void policyQuery.refetch();
            }}
            disabled={healthQuery.isFetching || policyQuery.isFetching}
            className="btn-secondary w-full sm:w-auto"
          >
            <RefreshCw
              className={`w-4 h-4 ${healthQuery.isFetching ? 'animate-spin' : ''}`}
              aria-hidden="true"
            />
            {t('platformAdmin.refreshHealth')}
          </button>
        }
      />

      {/* Platform 2FA Policy */}
      <section className="card space-y-5">
        <div className="flex items-start gap-3">
          <div className="w-10 h-10 rounded-xl bg-[var(--accent-soft)] flex items-center justify-center shrink-0">
            <ShieldCheck className="w-5 h-5 text-[var(--accent-1)]" aria-hidden="true" />
          </div>
          <div>
            <h2 className="text-base font-semibold text-gray-900 dark:text-slate-100">
              {t('platformAdmin.twoFactorPolicy.title')}
            </h2>
            <p className="text-sm text-gray-500 dark:text-slate-400">
              {t('platformAdmin.twoFactorPolicy.description')}
            </p>
          </div>
        </div>

        {policyQuery.isLoading ? (
          <div role="status" aria-label={t('common.loading')} className="space-y-4 py-2">
            <Skeleton className="h-8 w-64" />
            <Skeleton className="h-11 w-64 rounded-xl" />
            <Skeleton className="h-4 w-48" />
          </div>
        ) : policyQuery.isError ? (
          <EmptyState
            compact
            icon={AlertTriangle}
            title={t('platformAdmin.twoFactorPolicy.loadFailed')}
            description={getErrorMessage(policyQuery.error, t('common.error'))}
            action={
              <button
                type="button"
                onClick={() => void policyQuery.refetch()}
                className="btn-secondary"
              >
                {t('common.retry')}
              </button>
            }
          />
        ) : (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              setPolicyError('');
              savePolicyMutation.mutate();
            }}
            className="space-y-4"
          >
            {policyError && (
              <div
                role="alert"
                className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-xl border border-red-100 dark:border-red-500/25"
              >
                {policyError}
              </div>
            )}

            <label className="flex items-center gap-2.5 min-h-11 py-1 text-sm font-medium text-gray-800 dark:text-slate-200 cursor-pointer">
              <input
                type="checkbox"
                checked={enforce}
                onChange={(e) => setEnforceDraft(e.target.checked)}
                className="rounded border-gray-300 dark:border-slate-600 text-[var(--accent-1)] focus:ring-[var(--ring)]"
              />
              {t('platformAdmin.twoFactorPolicy.enforceLabel')}
            </label>

            <div className="max-w-xs">
              <FormField
                label={t('platformAdmin.twoFactorPolicy.graceUntilLabel')}
                hint={t('platformAdmin.twoFactorPolicy.graceUntilHint')}
              >
                {(aria) => (
                  <input
                    {...aria}
                    type="date"
                    value={graceUntil}
                    onChange={(e) => setGraceUntilDraft(e.target.value)}
                    className="form-input"
                  />
                )}
              </FormField>
            </div>

            {(policyQuery.data?.non_compliant.length ?? 0) === 0 ? (
              <p className="text-sm text-emerald-700 dark:text-emerald-300">
                {t('platformAdmin.twoFactorPolicy.allCompliant')}
              </p>
            ) : (
              <div className="rounded-xl border border-amber-200 dark:border-amber-500/30 bg-amber-50 dark:bg-amber-500/10 p-4 space-y-2">
                <p className="text-sm font-medium text-amber-800 dark:text-amber-300">
                  {t('platformAdmin.twoFactorPolicy.nonCompliantTitle', {
                    count: policyQuery.data?.non_compliant.length ?? 0,
                  })}
                </p>
                <ul className="list-disc list-inside text-xs text-amber-800 dark:text-amber-200 space-y-1">
                  {policyQuery.data?.non_compliant.map((email) => (
                    <li key={email}>{email}</li>
                  ))}
                </ul>
              </div>
            )}

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 max-w-xl pt-2 border-t border-gray-100 dark:border-slate-800">
              <FormField label={t('users.confirmWithYourPassword')} required>
                {(aria) => (
                  <input
                    {...aria}
                    type="password"
                    autoComplete="current-password"
                    value={actorPassword}
                    onChange={(e) => setActorPassword(e.target.value)}
                    placeholder={t('auth.twoFactor.currentPasswordPlaceholder')}
                    className="form-input"
                    required
                  />
                )}
              </FormField>
              {twoFactorEnabled && (
                <FormField label={t('auth.twoFactor.title')} required>
                  {(aria) => (
                    <input
                      {...aria}
                      type="text"
                      inputMode="text"
                      autoComplete="one-time-code"
                      value={actorTotpCode}
                      onChange={(e) => setActorTotpCode(e.target.value)}
                      placeholder="123456"
                      className="form-input text-center tracking-widest"
                      required
                    />
                  )}
                </FormField>
              )}
            </div>

            <div className="flex justify-end">
              <button
                type="submit"
                disabled={
                  savePolicyMutation.isPending ||
                  !actorPassword ||
                  (twoFactorEnabled && !actorTotpCode.trim())
                }
                className="btn-primary w-full sm:w-auto inline-flex items-center justify-center gap-2"
              >
                {savePolicyMutation.isPending ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                    {t('common.saving')}
                  </>
                ) : (
                  t('platformAdmin.twoFactorPolicy.savePolicy')
                )}
              </button>
            </div>
          </form>
        )}
      </section>

      {/* Operational Health */}
      <section className="card space-y-5">
        <h2 className="text-base font-semibold text-gray-900 dark:text-slate-100">
          {t('platformAdmin.health.title')}
        </h2>

        {healthQuery.isLoading ? (
          <div role="status" aria-label={t('common.loading')} className="space-y-4 py-2">
            <Skeleton className="h-20 w-full rounded-xl" />
            <div className="grid grid-cols-2 sm:grid-cols-3 xl:grid-cols-5 gap-3">
              {[0, 1, 2, 3, 4].map((i) => (
                <Skeleton key={i} className="h-20 w-full rounded-xl" />
              ))}
            </div>
          </div>
        ) : healthQuery.isError || !healthQuery.data ? (
          <EmptyState
            compact
            icon={AlertTriangle}
            title={t('platformAdmin.health.loadFailed')}
            description={getErrorMessage(healthQuery.error, t('common.error'))}
            action={
              <button
                type="button"
                onClick={() => void healthQuery.refetch()}
                className="btn-secondary"
              >
                {t('common.retry')}
              </button>
            }
          />
        ) : (
          <div className="space-y-6">
            <div className="rounded-xl border border-gray-100 dark:border-slate-800 bg-slate-50/80 dark:bg-slate-800/40 grid grid-cols-1 sm:grid-cols-3 divide-y sm:divide-y-0 sm:divide-x divide-gray-200/70 dark:divide-slate-700/60">
              <div className="p-4">
                <p className="text-xs text-gray-500 dark:text-slate-400">
                  {t('platformAdmin.health.version')}
                </p>
                <p className="text-base font-semibold text-gray-900 dark:text-slate-100 mt-1 tabular-nums">
                  {healthQuery.data.version}
                </p>
              </div>
              <div className="p-4">
                <p className="text-xs text-gray-500 dark:text-slate-400">
                  {t('platformAdmin.health.revision')}
                </p>
                <p className="text-base font-semibold text-gray-900 dark:text-slate-100 mt-1 font-mono truncate">
                  {healthQuery.data.revision}
                </p>
              </div>
              <div className="p-4">
                <p className="text-xs text-gray-500 dark:text-slate-400">
                  {t('platformAdmin.health.migrations')}
                </p>
                <p className="text-base font-semibold text-gray-900 dark:text-slate-100 mt-1 tabular-nums">
                  {healthQuery.data.latest_migration_version ?? '—'} (
                  {t('platformAdmin.health.migrationCount', {
                    count: healthQuery.data.migration_count,
                  })}
                  )
                </p>
              </div>
            </div>

            <div className="grid grid-cols-2 sm:grid-cols-3 xl:grid-cols-5 gap-3">
              {[
                {
                  key: 'emailPending',
                  label: t('platformAdmin.health.emailPending'),
                  value: healthQuery.data.queues.email_outbox_pending,
                  alert: false,
                },
                {
                  key: 'emailFailed',
                  label: t('platformAdmin.health.emailFailed24h'),
                  value: healthQuery.data.queues.email_outbox_failed_24h,
                  alert: healthQuery.data.queues.email_outbox_failed_24h > 0,
                },
                {
                  key: 'pushPending',
                  label: t('platformAdmin.health.pushPending'),
                  value: healthQuery.data.queues.push_outbox_pending,
                  alert: false,
                },
                {
                  key: 'pushFailed',
                  label: t('platformAdmin.health.pushFailed24h'),
                  value: healthQuery.data.queues.push_outbox_failed_24h,
                  alert: healthQuery.data.queues.push_outbox_failed_24h > 0,
                },
                {
                  key: 'jobsFailed',
                  label: t('platformAdmin.health.jobsFailed24h'),
                  value: healthQuery.data.queues.background_jobs_failed_24h,
                  alert: healthQuery.data.queues.background_jobs_failed_24h > 0,
                },
              ].map((metric) => (
                <div
                  key={metric.key}
                  className={`rounded-xl border p-4 ${
                    metric.alert
                      ? 'border-red-200 dark:border-red-500/30 bg-red-50/60 dark:bg-red-500/10'
                      : 'border-gray-100 dark:border-slate-800'
                  }`}
                >
                  <p
                    className={`text-xs ${
                      metric.alert
                        ? 'text-red-700 dark:text-red-300 font-medium'
                        : 'text-gray-500 dark:text-slate-400'
                    }`}
                  >
                    {metric.label}
                  </p>
                  <p
                    className={`text-2xl font-bold mt-1 tabular-nums ${
                      metric.alert
                        ? 'text-red-700 dark:text-red-300'
                        : 'text-gray-900 dark:text-slate-100'
                    }`}
                  >
                    {metric.value}
                  </p>
                </div>
              ))}
            </div>

            <div className="space-y-3">
              <h3 className="text-xs font-semibold uppercase tracking-wider text-gray-600 dark:text-slate-400">
                {t('platformAdmin.health.jobsTitle')}
              </h3>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                {jobRows.map(({ key, label, health }) => (
                  <div
                    key={key}
                    className="rounded-xl border border-gray-100 dark:border-slate-800 p-4 flex items-start justify-between gap-3"
                  >
                    <div className="space-y-0.5 min-w-0">
                      <p className="text-sm font-medium text-gray-900 dark:text-slate-100">
                        {label}
                      </p>
                      <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-gray-500 dark:text-slate-400">
                        {health.last_run_at && (
                          <span>
                            {t('platformAdmin.health.lastRun', {
                              time: formatDateTime(health.last_run_at),
                            })}
                          </span>
                        )}
                        {health.processed != null && (
                          <span>
                            {t('platformAdmin.health.processed', { count: health.processed })}
                          </span>
                        )}
                      </div>
                      {health.last_error && (
                        <p className="text-xs text-red-600 dark:text-red-400 break-words">
                          {health.last_error}
                        </p>
                      )}
                    </div>
                    <JobStatusBadge job={health} />
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
