import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ShieldCheck, ShieldOff, Copy, Download, Check, X } from 'lucide-react';
import {
  totpStatus,
  totpSetupBegin,
  totpSetupConfirm,
  totpDisable,
  totpRegenerateBackupCodes,
} from '@/api/totp';
import { getErrorMessage } from '@/lib/utils';

type View = 'idle' | 'setting_up' | 'backup_codes' | 'disabling' | 'regenerating';

export function TwoFactorSetup() {
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
    onError: (err: unknown) => setError(getErrorMessage(err, 'Failed to start 2FA setup')),
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
    onError: (err: unknown) => setError(getErrorMessage(err, 'Invalid code')),
  });

  const disableMutation = useMutation({
    mutationFn: (pw: string) => totpDisable(pw),
    onSuccess: () => {
      setError('');
      setPassword('');
      setView('idle');
      setSuccess('2FA disabled');
      setTimeout(() => setSuccess(''), 3000);
      queryClient.invalidateQueries({ queryKey: ['totp-status'] });
    },
    onError: (err: unknown) => setError(getErrorMessage(err, 'Failed to disable 2FA')),
  });

  const regenerateMutation = useMutation({
    mutationFn: (pw: string) => totpRegenerateBackupCodes(pw),
    onSuccess: (data) => {
      setError('');
      setPassword('');
      setBackupCodes(data.backup_codes);
      setView('backup_codes');
    },
    onError: (err: unknown) => setError(getErrorMessage(err, 'Failed to regenerate backup codes')),
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
          <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-900" />
        </div>
      </div>
    );
  }

  return (
    <div className="card">
      <div className="section-header">
        <div className="flex items-center gap-2">
          <ShieldCheck className="w-4 h-4 text-gray-400" />
          <span className="section-title">Two-Factor Authentication</span>
        </div>
      </div>

      <p className="text-sm text-gray-500 mb-4">
        Add an extra layer of security using an authenticator app (Google Authenticator, Authy, 1Password, etc.).
      </p>

      {error && (
        <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-xl mb-4">{error}</div>
      )}
      {success && (
        <div className="bg-green-50 text-green-600 text-sm px-4 py-3 rounded-xl mb-4">{success}</div>
      )}

      {view === 'backup_codes' && (
        <div className="space-y-4">
          <div className="bg-amber-50 text-amber-800 text-sm px-4 py-3 rounded-xl">
            Save these backup codes somewhere safe. Each can be used once to sign in if you lose access
            to your authenticator app. They won't be shown again.
          </div>
          <div className="grid grid-cols-2 gap-2 p-4 bg-gray-50 rounded-xl font-mono text-sm">
            {backupCodes.map((code) => (
              <div key={code}>{code}</div>
            ))}
          </div>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={handleCopyBackupCodes}
              className="flex items-center gap-2 text-sm font-medium text-gray-700 hover:text-gray-900 border border-gray-300 rounded-lg px-3 py-2"
            >
              {copied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
              {copied ? 'Copied' : 'Copy'}
            </button>
            <button
              type="button"
              onClick={handleDownloadBackupCodes}
              className="flex items-center gap-2 text-sm font-medium text-gray-700 hover:text-gray-900 border border-gray-300 rounded-lg px-3 py-2"
            >
              <Download className="w-4 h-4" />
              Download
            </button>
          </div>
          <button
            type="button"
            onClick={() => setView('idle')}
            className="w-full bg-black text-white py-2.5 rounded-xl font-semibold hover:bg-gray-800 transition-all"
          >
            I've saved these codes
          </button>
        </div>
      )}

      {view === 'setting_up' && beginMutation.data && (
        <div className="space-y-4">
          <div className="flex justify-center">
            <img
              src={`data:image/png;base64,${beginMutation.data.qr_code_base64}`}
              alt="2FA setup QR code"
              className="rounded-xl border border-gray-200"
              width={200}
              height={200}
            />
          </div>
          <p className="text-xs text-gray-500 text-center">
            Can't scan? Enter this code manually:{' '}
            <span className="font-mono text-gray-700">{beginMutation.data.secret}</span>
          </p>
          <div>
            <label className="form-label">Enter the 6-digit code to confirm</label>
            <input
              type="text"
              inputMode="numeric"
              autoComplete="one-time-code"
              value={confirmCode}
              onChange={(e) => setConfirmCode(e.target.value)}
              className="border p-2.5 rounded-lg w-full text-sm text-center tracking-widest outline-none focus:border-black transition-colors"
              placeholder="123456"
              autoFocus
            />
          </div>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => confirmMutation.mutate(confirmCode.trim())}
              disabled={!confirmCode.trim() || confirmMutation.isPending}
              className="flex-1 bg-black text-white py-2.5 rounded-xl font-semibold hover:bg-gray-800 disabled:opacity-50 transition-all"
            >
              {confirmMutation.isPending ? 'Verifying...' : 'Confirm & Enable'}
            </button>
            <button
              type="button"
              onClick={() => {
                setView('idle');
                setConfirmCode('');
                setError('');
              }}
              className="px-4 py-2.5 text-sm font-medium text-gray-500 hover:text-gray-700"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {(view === 'disabling' || view === 'regenerating') && (
        <div className="space-y-3">
          <label className="form-label">Confirm your password to continue</label>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className="border p-2.5 rounded-lg w-full text-sm outline-none focus:border-black transition-colors"
            placeholder="Current password"
            autoFocus
          />
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() =>
                view === 'disabling'
                  ? disableMutation.mutate(password)
                  : regenerateMutation.mutate(password)
              }
              disabled={!password || disableMutation.isPending || regenerateMutation.isPending}
              className="flex-1 bg-black text-white py-2.5 rounded-xl font-semibold hover:bg-gray-800 disabled:opacity-50 transition-all"
            >
              {disableMutation.isPending || regenerateMutation.isPending ? 'Confirming...' : 'Confirm'}
            </button>
            <button
              type="button"
              onClick={() => {
                setView('idle');
                setPassword('');
                setError('');
              }}
              className="px-4 py-2.5 text-sm font-medium text-gray-500 hover:text-gray-700"
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
              <div className="flex items-center gap-2 text-sm text-green-700 bg-green-50 px-4 py-3 rounded-xl">
                <ShieldCheck className="w-4 h-4 shrink-0" />
                Two-factor authentication is enabled
              </div>
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => setView('regenerating')}
                  className="text-sm font-medium text-gray-700 hover:text-gray-900 border border-gray-300 rounded-lg px-3 py-2"
                >
                  Regenerate backup codes
                </button>
                <button
                  type="button"
                  onClick={() => setView('disabling')}
                  className="flex items-center gap-2 text-sm font-medium text-red-600 hover:text-red-700 border border-gray-300 rounded-lg px-3 py-2"
                >
                  <ShieldOff className="w-4 h-4" />
                  Disable 2FA
                </button>
              </div>
            </div>
          ) : (
            <button
              type="button"
              onClick={() => beginMutation.mutate()}
              disabled={beginMutation.isPending}
              className="flex items-center gap-2 text-sm font-medium text-black hover:text-gray-600 transition-colors"
            >
              <ShieldCheck className="w-4 h-4" />
              {beginMutation.isPending ? 'Starting...' : 'Enable 2FA'}
            </button>
          )}
        </>
      )}
    </div>
  );
}
