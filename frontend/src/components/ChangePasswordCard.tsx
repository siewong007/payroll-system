import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff, KeyRound } from 'lucide-react';
import api from '@/api/client';
import { useAuth } from '@/context/AuthContext';
import { passwordPolicyHint, validatePassword } from '@/lib/password';
import { getErrorMessage } from '@/lib/utils';

export function ChangePasswordCard() {
  const { user, logout } = useAuth();
  const { t } = useTranslation();
  const navigate = useNavigate();
  const twoFactorEnabled = Boolean(user?.two_factor_enabled);
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [showCurrent, setShowCurrent] = useState(false);
  const [showNew, setShowNew] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (newPassword !== confirmPassword) {
      setError(t('auth.passwordCard.mismatch'));
      return;
    }
    const policyError = validatePassword(newPassword);
    if (policyError) {
      setError(policyError);
      return;
    }

    setLoading(true);
    try {
      await api.put('/auth/change-password', {
        current_password: currentPassword,
        new_password: newPassword,
        ...(twoFactorEnabled && totpCode.trim() ? { totp_code: totpCode.trim() } : {}),
      });
      // The backend revokes every session on password change — including this
      // one — so skip the authenticated DELETE call and just clear local/cookie state.
      await logout({ skipRemotePushDelete: true });
      navigate('/login');
    } catch (err) {
      setError(getErrorMessage(err, t('auth.passwordCard.failed')));
      setLoading(false);
    }
  };

  return (
    <section className="card">
      <div className="section-header">
        <div className="flex items-center gap-2">
          <KeyRound className="w-4 h-4 text-[var(--muted-foreground)]" />
          <span className="section-title">{t('auth.passwordCard.title')}</span>
        </div>
      </div>

      <p className="text-sm text-[var(--muted-foreground)] mb-4">
        {t('auth.passwordCard.description')}
      </p>

      {error && (
        <div role="alert" className="bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 text-red-700 dark:text-red-300 text-sm px-3.5 py-2.5 rounded-lg mb-4">
          {error}
        </div>
      )}

      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="form-label" htmlFor="cp-current">{t('auth.passwordCard.current')}</label>
          <div className="relative">
            <input
              id="cp-current"
              type={showCurrent ? 'text' : 'password'}
              autoComplete="current-password"
              className="form-input pr-11"
              value={currentPassword}
              onChange={(e) => setCurrentPassword(e.target.value)}
              required
            />
            <button
              type="button"
              onClick={() => setShowCurrent((v) => !v)}
              aria-label={showCurrent ? t('a11y.hidePassword') : t('a11y.showPassword')}
              aria-pressed={showCurrent}
              className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-400 dark:text-slate-400 hover:text-gray-600 dark:hover:text-slate-200 transition-colors"
            >
              {showCurrent ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            </button>
          </div>
        </div>
        <div>
          <label className="form-label" htmlFor="cp-new">{t('auth.passwordCard.new')}</label>
          <div className="relative">
            <input
              id="cp-new"
              type={showNew ? 'text' : 'password'}
              autoComplete="new-password"
              className="form-input pr-11"
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
              required
            />
            <button
              type="button"
              onClick={() => setShowNew((v) => !v)}
              aria-label={showNew ? t('a11y.hidePassword') : t('a11y.showPassword')}
              aria-pressed={showNew}
              className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-400 dark:text-slate-400 hover:text-gray-600 dark:hover:text-slate-200 transition-colors"
            >
              {showNew ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            </button>
          </div>
          <p className="text-xs text-gray-500 dark:text-slate-400 mt-1">{passwordPolicyHint()}</p>
        </div>
        <div>
          <label className="form-label" htmlFor="cp-confirm">{t('auth.passwordCard.confirm')}</label>
          <div className="relative">
            <input
              id="cp-confirm"
              type={showConfirm ? 'text' : 'password'}
              autoComplete="new-password"
              className="form-input pr-11"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              required
            />
            <button
              type="button"
              onClick={() => setShowConfirm((v) => !v)}
              aria-label={showConfirm ? t('a11y.hidePassword') : t('a11y.showPassword')}
              aria-pressed={showConfirm}
              className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-400 dark:text-slate-400 hover:text-gray-600 dark:hover:text-slate-200 transition-colors"
            >
              {showConfirm ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            </button>
          </div>
        </div>
        {twoFactorEnabled && (
          <div>
            <label className="form-label" htmlFor="cp-totp">{t('auth.twoFactor.title')}</label>
            <input
              id="cp-totp"
              type="text"
              inputMode="text"
              autoComplete="one-time-code"
              className="form-input text-center tracking-widest"
              placeholder="123456"
              value={totpCode}
              onChange={(e) => setTotpCode(e.target.value)}
              required
            />
          </div>
        )}
        <div className="flex justify-end">
          <button
            type="submit"
            disabled={loading || (twoFactorEnabled && !totpCode.trim())}
            className="btn-primary"
          >
            {loading ? t('auth.passwordCard.submitting') : t('auth.passwordCard.submit')}
          </button>
        </div>
      </form>
    </section>
  );
}
