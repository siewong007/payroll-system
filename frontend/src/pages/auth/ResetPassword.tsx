import { useState, useEffect } from 'react';
import { Link, useLocation, useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff, Loader2 } from 'lucide-react';
import { resetPassword, validateResetToken } from '@/api/admin';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { Skeleton } from '@/components/ui/Skeleton';
import { AuthLayout } from '@/components/layout/AuthLayout';
import { TurnstileWidget } from '@/components/TurnstileWidget';
import { PASSWORD_MAX_BYTES, PASSWORD_MIN_LENGTH, passwordPolicyHint, validatePassword } from '@/lib/password';
import { useTurnstileWaiter } from '@/lib/useTurnstileWaiter';
import { getErrorMessage } from '@/lib/utils';

export function ResetPassword() {
  const { t } = useTranslation();
  const location = useLocation();
  const [searchParams] = useSearchParams();
  const [token] = useState(() => {
    const hashParams = new URLSearchParams(
      (location.hash || window.location.hash).replace(/^#/, ''),
    );
    const hashToken = hashParams.get('token');
    if (hashToken) {
      return hashToken;
    }
    return (
      searchParams.get('token') ||
      new URLSearchParams(window.location.search).get('token') ||
      ''
    );
  });
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState(false);
  const [loading, setLoading] = useState(false);
  const [validating, setValidating] = useState(true);
  const [tokenValid, setTokenValid] = useState(false);
  const {
    widgetRef: turnstileRef,
    waitForToken: waitForTurnstileToken,
    handleVerify: handleTurnstileVerify,
    handleExpire: handleTurnstileExpire,
    handleError: handleTurnstileError,
    resetWidget: resetTurnstileWidget,
  } = useTurnstileWaiter();

  useEffect(() => {
    if (
      new URLSearchParams(window.location.hash.replace(/^#/, '')).has('token') ||
      new URLSearchParams(window.location.search).has('token')
    ) {
      window.history.replaceState(null, '', window.location.pathname);
    }
  }, []);

  useEffect(() => {
    if (!token) {
      setValidating(false);
      return;
    }
    validateResetToken(token)
      .then(() => setTokenValid(true))
      .catch(() => setTokenValid(false))
      .finally(() => setValidating(false));
  }, [token]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    const policyError = validatePassword(password);
    if (policyError) {
      setError(policyError);
      return;
    }
    if (password !== confirmPassword) {
      setError(t('auth.reset.mismatch'));
      return;
    }

    setLoading(true);
    try {
      const captcha = await waitForTurnstileToken();
      await resetPassword(token, password, captcha);
      setSuccess(true);
    } catch (err: unknown) {
      resetTurnstileWidget();
      setError(getErrorMessage(err, t('auth.reset.failed')));
    } finally {
      setLoading(false);
    }
  };

  if (validating) {
    return (
      <AuthLayout>
        <div
          role="status"
          className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow p-6 sm:p-8 text-center space-y-5"
        >
          <BrandLogo variant="lockup-dark" priority className="h-10 w-auto mx-auto rounded-xl" />
          <div className="space-y-3 py-2">
            <Skeleton className="h-4 w-40 mx-auto" />
            <Skeleton className="h-10 w-full rounded-xl" />
            <Skeleton className="h-10 w-full rounded-xl" />
          </div>
          <p className="text-sm text-gray-600 dark:text-slate-300">{t('auth.reset.validating')}</p>
        </div>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout>
      <div className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow p-6 sm:p-8">
        <div className="text-center mb-8">
          <BrandLogo variant="lockup-dark" priority className="h-10 w-auto mx-auto mb-4 rounded-xl" />
          <h1 className="text-xl font-semibold text-gray-900 dark:text-slate-100">{t('auth.reset.title')}</h1>
        </div>

        {!token || !tokenValid ? (
          <div role="alert" className="text-center space-y-4">
            <div className="w-16 h-16 bg-red-50 dark:bg-red-500/15 rounded-full flex items-center justify-center mx-auto">
              <svg className="w-8 h-8 text-red-600 dark:text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </div>
            <p className="text-sm text-gray-600 dark:text-slate-300">
              {t('auth.reset.invalidLink')}
            </p>
            <Link
              to="/forgot-password"
              className="inline-flex min-h-11 items-center justify-center px-3 text-sm text-black dark:text-indigo-400 font-medium hover:underline rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
            >
              {t('auth.reset.requestNew')}
            </Link>
          </div>
        ) : success ? (
          <div role="status" className="text-center space-y-4">
            <div className="w-16 h-16 bg-green-50 dark:bg-emerald-500/15 rounded-full flex items-center justify-center mx-auto">
              <svg className="w-8 h-8 text-green-600 dark:text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <p className="text-sm text-gray-600 dark:text-slate-300">
              {t('auth.reset.success')}
            </p>
            <Link
              to="/login"
              className="inline-flex min-h-11 items-center justify-center px-3 text-sm text-black dark:text-indigo-400 font-medium hover:underline rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
            >
              {t('auth.reset.signInNew')}
            </Link>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="space-y-5">
            {error && (
              <div role="alert" className="bg-red-50 dark:bg-red-500/10 border border-red-100 dark:border-red-500/30 text-red-700 dark:text-red-300 text-sm px-4 py-3 rounded-xl">
                {error}
              </div>
            )}

            <div>
              <label className="form-label" htmlFor="reset-password">{t('auth.reset.newPassword')}</label>
              <div className="relative">
                <input
                  id="reset-password"
                  type={showPassword ? 'text' : 'password'}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="form-input pr-11"
                  placeholder={t('auth.reset.newPasswordPlaceholder')}
                  autoComplete="new-password"
                  required
                  minLength={PASSWORD_MIN_LENGTH}
                  maxLength={PASSWORD_MAX_BYTES}
                />
                <button
                  type="button"
                  onClick={() => setShowPassword((v) => !v)}
                  aria-label={showPassword ? t('a11y.hidePassword') : t('a11y.showPassword')}
                  aria-pressed={showPassword}
                  className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] rounded-r-xl transition-colors"
                >
                  {showPassword ? <EyeOff className="w-4 h-4" aria-hidden="true" /> : <Eye className="w-4 h-4" aria-hidden="true" />}
                </button>
              </div>
              <p className="text-xs text-gray-600 dark:text-slate-300 mt-1">{passwordPolicyHint()}</p>
            </div>

            <div>
              <label className="form-label" htmlFor="reset-confirm">{t('auth.reset.confirmPassword')}</label>
              <div className="relative">
                <input
                  id="reset-confirm"
                  type={showConfirm ? 'text' : 'password'}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  className="form-input pr-11"
                  placeholder={t('auth.reset.confirmPlaceholder')}
                  autoComplete="new-password"
                  required
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

            <div className="flex justify-center">
              <TurnstileWidget
                ref={turnstileRef}
                onVerify={handleTurnstileVerify}
                onExpire={handleTurnstileExpire}
                onError={handleTurnstileError}
              />
            </div>

            <button
              type="submit"
              disabled={loading}
              className="btn-primary w-full inline-flex items-center justify-center gap-2"
            >
              {loading ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                  {t('auth.reset.submitting')}
                </>
              ) : (
                t('auth.reset.submit')
              )}
            </button>
          </form>
        )}
      </div>
    </AuthLayout>
  );
}
