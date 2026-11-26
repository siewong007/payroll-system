import { useState } from 'react';
import { Navigate, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff, Loader2, LogOut } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import api from '@/api/client';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { Skeleton } from '@/components/ui/Skeleton';
import { AuthLayout } from '@/components/layout/AuthLayout';
import { PASSWORD_MAX_BYTES, PASSWORD_MIN_LENGTH, passwordPolicyHint, validatePassword } from '@/lib/password';
import { getErrorMessage } from '@/lib/utils';

export function ChangePassword() {
  const { user, logout, isAuthenticated, isLoading } = useAuth();
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showCurrent, setShowCurrent] = useState(false);
  const [showNew, setShowNew] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  if (isLoading) {
    return (
      <AuthLayout>
        <div
          role="status"
          aria-label={t('common.loading')}
          className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow-lg p-6 sm:p-8 space-y-5"
        >
          <BrandLogo variant="lockup-dark" priority className="h-10 w-auto mx-auto rounded-xl" />
          <div className="space-y-3 py-2">
            <Skeleton className="h-4 w-44 mx-auto" />
            <Skeleton className="h-10 w-full rounded-xl" />
            <Skeleton className="h-10 w-full rounded-xl" />
            <Skeleton className="h-10 w-full rounded-xl" />
          </div>
        </div>
      </AuthLayout>
    );
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  const handleSignOut = async () => {
    await logout();
    navigate('/login', { replace: true });
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (newPassword !== confirmPassword) {
      setError(t('auth.change.mismatch'));
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
      });

      // Update local user state to clear the flag
      if (user) {
        const updatedUser = { ...user, must_change_password: false };
        localStorage.setItem('user', JSON.stringify(updatedUser));
      }

      // Logout so user logs in with new password (token is already revoked server-side)
      await logout({ skipRemotePushDelete: true });
      navigate('/login');
    } catch (err: unknown) {
      setError(getErrorMessage(err, t('auth.change.failed')));
    } finally {
      setLoading(false);
    }
  };

  return (
    <AuthLayout
      footerExtra={
        <div className="pb-2">
          <button
            type="button"
            onClick={() => void handleSignOut()}
            className="inline-flex min-h-11 items-center justify-center gap-1.5 rounded-xl px-4 text-sm font-medium text-gray-700 dark:text-slate-200 hover:bg-gray-200/70 dark:hover:bg-slate-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors"
          >
            <LogOut className="w-4 h-4" aria-hidden="true" />
            {t('nav.signOut')}
          </button>
        </div>
      }
    >
      <div className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow-lg p-6 sm:p-8">
        <div className="text-center mb-6">
          <BrandLogo variant="lockup-dark" priority className="h-10 w-auto mx-auto mb-4 rounded-xl" />
          <div className="w-12 h-12 bg-amber-100 dark:bg-amber-500/15 rounded-full flex items-center justify-center mx-auto mb-3">
            <svg className="w-6 h-6 text-amber-600 dark:text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
            </svg>
          </div>
          <h1 className="text-xl font-bold text-gray-900 dark:text-slate-100">{t('auth.change.title')}</h1>
          <p className="text-sm text-gray-600 dark:text-slate-300 mt-1">
            {t('auth.change.body')}
          </p>
          <p className="text-xs text-gray-600 dark:text-slate-300 mt-2">
            {passwordPolicyHint()}
          </p>
        </div>

        {error && (
          <div role="alert" className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-lg border border-red-100 dark:border-red-500/30 mb-4">
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="form-label" htmlFor="cpw-current">{t('auth.change.current')}</label>
            <div className="relative">
              <input
                id="cpw-current"
                type={showCurrent ? 'text' : 'password'}
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
                className="form-input pr-11"
                autoComplete="current-password"
                required
                placeholder={t('auth.change.currentPlaceholder')}
              />
              <button
                type="button"
                onClick={() => setShowCurrent((v) => !v)}
                aria-label={showCurrent ? t('a11y.hidePassword') : t('a11y.showPassword')}
                aria-pressed={showCurrent}
                className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] rounded-r-xl transition-colors"
              >
                {showCurrent ? <EyeOff className="w-4 h-4" aria-hidden="true" /> : <Eye className="w-4 h-4" aria-hidden="true" />}
              </button>
            </div>
          </div>
          <div>
            <label className="form-label" htmlFor="cpw-new">{t('auth.change.new')}</label>
            <div className="relative">
              <input
                id="cpw-new"
                type={showNew ? 'text' : 'password'}
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                className="form-input pr-11"
                autoComplete="new-password"
                required
                minLength={PASSWORD_MIN_LENGTH}
                maxLength={PASSWORD_MAX_BYTES}
                placeholder={t('auth.change.newPlaceholder')}
              />
              <button
                type="button"
                onClick={() => setShowNew((v) => !v)}
                aria-label={showNew ? t('a11y.hidePassword') : t('a11y.showPassword')}
                aria-pressed={showNew}
                className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] rounded-r-xl transition-colors"
              >
                {showNew ? <EyeOff className="w-4 h-4" aria-hidden="true" /> : <Eye className="w-4 h-4" aria-hidden="true" />}
              </button>
            </div>
          </div>
          <div>
            <label className="form-label" htmlFor="cpw-confirm">{t('auth.change.confirm')}</label>
            <div className="relative">
              <input
                id="cpw-confirm"
                type={showConfirm ? 'text' : 'password'}
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                className="form-input pr-11"
                autoComplete="new-password"
                required
                placeholder={t('auth.change.confirmPlaceholder')}
              />
              <button
                type="button"
                onClick={() => setShowConfirm((v) => !v)}
                aria-label={showConfirm ? t('a11y.hidePassword') : t('a11y.showPassword')}
                aria-pressed={showConfirm}
                className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] rounded-r-xl transition-colors"
              >
                {showConfirm ? <EyeOff className="w-4 h-4" aria-hidden="true" /> : <Eye className="w-4 h-4" aria-hidden="true" />}
              </button>
            </div>
          </div>
          <button
            type="submit"
            disabled={loading}
            className="btn-primary w-full inline-flex items-center justify-center gap-2"
          >
            {loading ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                {t('auth.change.submitting')}
              </>
            ) : (
              t('auth.change.submit')
            )}
          </button>
        </form>
      </div>
    </AuthLayout>
  );
}
