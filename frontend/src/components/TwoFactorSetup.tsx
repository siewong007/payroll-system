import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ShieldCheck, ShieldOff, Copy, Download, Check, X } from 'lucide-react';
import {
  totpStatus,
  totpSetupBegin,
  totpSetupConfirm,
  totpDisable,
  totpRegenerateBackupCodes,
} from '@/api/totp';
import { FormField } from '@/components/ui/FormField';
import { getErrorMessage } from '@/lib/utils';

type View = 'idle' | 'setting_up' | 'backup_codes' | 'disabling' | 'regenerating';

export function TwoFactorSetup() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [view, setView] = useState<View>('idle');
  const [confirmCode, setConfirmCode] = useState('');
  const [password, setPassword] = useState('');
  const [backupCodes, setBackupCodes] = useState<string[]>([]);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [copied, setCopied] = useState(false);

  const { data: status, isLoading } = useQuery({
    queryKey: ['totp-status'],
    queryFn: totpStatus,
  });

  const beginMutation = useMutation({
    mutationFn: totpSetupBegin,
    onSuccess: () => {
      setError('');
      setView('setting_up');
    },
    onError: (err: unknown) => setError(getErrorMessage(err, t('auth.twoFactor.setupFailed'))),
  });

  const confirmMutation = useMutation({
    mutationFn: (code: string) => totpSetupConfirm(code),
    onSuccess: (data) => {
      setError('');
      setBackupCodes(data.backup_codes);
      setView('backup_codes');
      setConfirmCode('');
      queryClient.invalidateQueries({ queryKey: ['totp-status'] });
    },
    onError: (err: unknown) => setError(getErrorMessage(err, t('auth.twoFactor.invalidCode'))),
  });

  const disableMutation = useMutation({
    mutationFn: (pw: string) => totpDisable(pw),
    onSuccess: () => {
      setError('');
      setPassword('');
      setView('idle');
      setSuccess(t('auth.twoFactor.disabled'));
      setTimeout(() => setSuccess(''), 3000);
      queryClient.invalidateQueries({ queryKey: ['totp-status'] });
    },
    onError: (err: unknown) => setError(getErrorMessage(err, t('auth.twoFactor.disableFailed'))),
  });

  const regenerateMutation = useMutation({
    mutationFn: (pw: string) => totpRegenerateBackupCodes(pw),
    onSuccess: (data) => {
      setError('');
      setPassword('');
      setBackupCodes(data.backup_codes);
      setView('backup_codes');
    },
    onError: (err: unknown) => setError(getErrorMessage(err, t('auth.twoFactor.regenFailed'))),
  });

  const handleCopyBackupCodes = () => {
    navigator.clipboard.writeText(backupCodes.join('\n'));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleDownloadBackupCodes = () => {
    const blob = new Blob([backupCodes.join('\n') + '\n'], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'payrollmy-backup-codes.txt';
    a.click();
    URL.revokeObjectURL(url);
  };

  if (isLoading) {
    return (
      <div className="card">
        <div className="flex justify-center py-8">
          <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-900 dark:border-slate-200" />
        </div>
      </div>
    );
  }

  return (
    <div className="card">
      <div className="section-header">
        <div className="flex items-center gap-2">
          <ShieldCheck className="w-4 h-4 text-gray-400 dark:text-slate-400" />
          <span className="section-title">{t('auth.twoFactor.setupTitle')}</span>
        </div>
        {status?.enabled ? (
          <span className="badge badge-approved ml-auto">{t('auth.twoFactor.enabledBadge')}</span>
        ) : (
          <span className="badge badge-cancelled ml-auto">{t('auth.twoFactor.offBadge')}</span>
        )}
      </div>

      <p className="text-sm text-gray-500 dark:text-slate-400 mb-4">
        {t('auth.twoFactor.description')}
      </p>

      {error && (
        <div role="alert" className="bg-red-50 dark:bg-red-500/10 border border-red-100 dark:border-red-500/30 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-xl mb-4">
          {error}
        </div>
      )}
      {success && (
        <div role="status" className="bg-green-50 dark:bg-emerald-500/10 border border-green-100 dark:border-emerald-500/30 text-green-600 dark:text-emerald-300 text-sm px-4 py-3 rounded-xl mb-4">
          {success}
        </div>
      )}

      {view === 'backup_codes' && (
        <div className="space-y-4">
          <div className="bg-amber-50 dark:bg-amber-500/15 border border-amber-200/70 dark:border-amber-500/30 text-amber-800 dark:text-amber-200 text-sm px-4 py-3 rounded-xl">
            {t('auth.twoFactor.backupWarning')}
          </div>
          <div className="grid grid-cols-2 gap-2 p-4 bg-gray-50 dark:bg-slate-800 text-gray-900 dark:text-slate-100 border border-gray-200/80 dark:border-slate-700 rounded-xl font-mono text-sm">
            {backupCodes.map((code) => (
              <div key={code}>{code}</div>
            ))}
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={handleCopyBackupCodes}
              className="min-h-10 flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-slate-200 hover:text-gray-900 dark:hover:text-white border border-gray-300 dark:border-slate-700 rounded-lg px-3 py-2"
            >
              {copied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
              {copied ? t('auth.twoFactor.copied') : t('auth.twoFactor.copy')}
            </button>
            <button
              type="button"
              onClick={handleDownloadBackupCodes}
              className="min-h-10 flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-slate-200 hover:text-gray-900 dark:hover:text-white border border-gray-300 dark:border-slate-700 rounded-lg px-3 py-2"
            >
              <Download className="w-4 h-4" />
              {t('auth.twoFactor.download')}
            </button>
          </div>
          <button
            type="button"
            onClick={() => setView('idle')}
            className="btn-primary w-full"
          >
            {t('auth.twoFactor.savedCodes')}
          </button>
        </div>
      )}

      {view === 'setting_up' && beginMutation.data && (
        <div className="space-y-4">
          <div className="flex justify-center">
            <img
              src={`data:image/png;base64,${beginMutation.data.qr_code_base64}`}
              alt={t('auth.twoFactor.qrAlt')}
              className="rounded-xl border border-gray-200 bg-white p-1"
              width={200}
              height={200}
            />
          </div>
          <p className="text-xs text-gray-500 dark:text-slate-400 text-center">
            {t('auth.twoFactor.manualEntry')}{' '}
            <span className="font-mono text-gray-700 dark:text-slate-200">{beginMutation.data.secret}</span>
          </p>
          <FormField label={t('auth.twoFactor.confirmLabel')}>
            {(aria) => (
              <input
                {...aria}
                type="text"
                inputMode="numeric"
                autoComplete="one-time-code"
                value={confirmCode}
                onChange={(e) => setConfirmCode(e.target.value)}
                className="form-input !p-2.5 text-center tracking-widest"
                placeholder="123456"
                autoFocus
              />
            )}
          </FormField>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => confirmMutation.mutate(confirmCode.trim())}
              disabled={!confirmCode.trim() || confirmMutation.isPending}
              className="btn-primary flex-1"
            >
              {confirmMutation.isPending ? t('auth.twoFactor.verifying') : t('auth.twoFactor.confirmEnable')}
            </button>
            <button
              type="button"
              onClick={() => {
                setView('idle');
                setConfirmCode('');
                setError('');
              }}
              className="btn-secondary"
            >
              {t('common.cancel')}
            </button>
          </div>
        </div>
      )}

      {(view === 'disabling' || view === 'regenerating') && (
        <div className="space-y-3">
          <FormField label={t('auth.twoFactor.confirmPasswordPrompt')}>
            {(aria) => (
              <input
                {...aria}
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="form-input !p-2.5"
                placeholder={t('auth.twoFactor.currentPasswordPlaceholder')}
                autoFocus
              />
            )}
          </FormField>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() =>
                view === 'disabling'
                  ? disableMutation.mutate(password)
                  : regenerateMutation.mutate(password)
              }
              disabled={!password || disableMutation.isPending || regenerateMutation.isPending}
              className="btn-primary flex-1"
            >
              {disableMutation.isPending || regenerateMutation.isPending ? t('auth.twoFactor.confirming') : t('auth.twoFactor.confirm')}
            </button>
            <button
              type="button"
              onClick={() => {
                setView('idle');
                setPassword('');
                setError('');
              }}
              aria-label={t('common.cancel')}
              className="min-h-11 min-w-11 inline-flex items-center justify-center text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 rounded-lg"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {view === 'idle' && (
        <>
          {status?.enabled ? (
            <div className="space-y-3">
              <div className="flex items-center gap-2 text-sm text-green-700 dark:text-emerald-300 bg-green-50 dark:bg-emerald-500/10 border border-green-100 dark:border-emerald-500/30 px-4 py-3 rounded-xl">
                <ShieldCheck className="w-4 h-4 shrink-0" />
                {t('auth.twoFactor.enabledBanner')}
              </div>
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => setView('regenerating')}
                  className="min-h-10 text-sm font-medium text-gray-700 dark:text-slate-200 hover:text-gray-900 dark:hover:text-white border border-gray-300 dark:border-slate-700 rounded-lg px-3 py-2"
                >
                  {t('auth.twoFactor.regenerate')}
                </button>
                <button
                  type="button"
                  onClick={() => setView('disabling')}
                  className="min-h-10 flex items-center gap-2 text-sm font-medium text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 border border-gray-300 dark:border-slate-700 rounded-lg px-3 py-2"
                >
                  <ShieldOff className="w-4 h-4" />
                  {t('auth.twoFactor.disable')}
                </button>
              </div>
            </div>
          ) : (
            <button
              type="button"
              onClick={() => beginMutation.mutate()}
              disabled={beginMutation.isPending}
              className="min-h-10 flex items-center gap-2 text-sm font-semibold text-[var(--accent-1)] hover:opacity-80 transition-opacity"
            >
              <ShieldCheck className="w-4 h-4" />
              {beginMutation.isPending ? t('auth.twoFactor.starting') : t('auth.twoFactor.enable')}
            </button>
          )}
        </>
      )}
    </div>
  );
}
