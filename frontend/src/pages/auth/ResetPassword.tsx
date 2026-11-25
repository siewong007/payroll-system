import { useState, useEffect, useRef } from 'react';
import { Link, useSearchParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff } from 'lucide-react';
import { resetPassword, validateResetToken } from '@/api/admin';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { LanguageSwitcher } from '@/components/ui/LanguageSwitcher';
import { TurnstileWidget, type TurnstileWidgetRef } from '@/components/TurnstileWidget';
import { PASSWORD_MAX_BYTES, PASSWORD_MIN_LENGTH, passwordPolicyHint, validatePassword } from '@/lib/password';

export function ResetPassword() {
  const { t } = useTranslation();
  const [searchParams] = useSearchParams();
  const token = searchParams.get('token') || '';
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState(false);
  const [loading, setLoading] = useState(false);
  const [validating, setValidating] = useState(true);
  const [tokenValid, setTokenValid] = useState(false);
  const turnstileRef = useRef<TurnstileWidgetRef>(null);
  const turnstileToken = useRef<string | undefined>(undefined);

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
    const captcha = turnstileToken.current;
    turnstileToken.current = undefined;
    try {
      await resetPassword(token, password, captcha);
      setSuccess(true);
    } catch {
      turnstileRef.current?.reset();
      setError(t('auth.reset.failed'));
    } finally {
      setLoading(false);
    }
  };

  if (validating) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-100 dark:bg-slate-950 px-4">
        <div className="text-gray-500 dark:text-slate-400">{t('auth.reset.validating')}</div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-100 dark:bg-slate-950 py-6">
      <div className="animate-page-enter w-full max-w-md px-4 space-y-5">
        <div className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow p-6 sm:p-8">
          <div className="text-center mb-8">
            <BrandLogo variant="lockup-dark" priority className="h-10 w-auto mx-auto mb-4 rounded-xl" />
            <h1 className="text-xl font-semibold text-gray-900 dark:text-slate-100">{t('auth.reset.title')}</h1>
          </div>

          {!token || !tokenValid ? (
            <div className="text-center space-y-4">
              <div className="w-16 h-16 bg-red-50 dark:bg-red-500/15 rounded-full flex items-center justify-center mx-auto">
                <svg className="w-8 h-8 text-red-500 dark:text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </div>
              <p className="text-sm text-gray-600 dark:text-slate-300">
                {t('auth.reset.invalidLink')}
              </p>
              <Link to="/forgot-password" className="inline-block text-sm text-black dark:text-indigo-400 font-medium hover:underline">
                {t('auth.reset.requestNew')}
              </Link>
            </div>
          ) : success ? (
            <div className="text-center space-y-4">
              <div className="w-16 h-16 bg-green-50 dark:bg-emerald-500/15 rounded-full flex items-center justify-center mx-auto">
                <svg className="w-8 h-8 text-green-500 dark:text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
              </div>
              <p className="text-sm text-gray-600 dark:text-slate-300">
                {t('auth.reset.success')}
              </p>
              <Link to="/login" className="inline-block text-sm text-black dark:text-indigo-400 font-medium hover:underline">
                {t('auth.reset.signInNew')}
              </Link>
            </div>
          ) : (
            <form onSubmit={handleSubmit} className="space-y-5">
              {error && (
                <div role="alert" className="bg-red-50 dark:bg-red-500/10 border border-red-100 dark:border-red-500/30 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-xl">
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
                    className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-400 dark:text-slate-400 hover:text-gray-600 dark:hover:text-slate-200 transition-colors"
                  >
                    {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                  </button>
                </div>
                <p className="text-xs text-gray-500 dark:text-slate-400 mt-1">{passwordPolicyHint()}</p>
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
                    className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-400 dark:text-slate-400 hover:text-gray-600 dark:hover:text-slate-200 transition-colors"
                  >
                    {showConfirm ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                  </button>
                </div>
              </div>

              <div className="flex justify-center">
                <TurnstileWidget
                  ref={turnstileRef}
                  onVerify={(t) => {
                    turnstileToken.current = t;
                  }}
                  onExpire={() => {
                    turnstileToken.current = undefined;
                  }}
                  onError={() => {
                    turnstileToken.current = undefined;
                  }}
                />
              </div>

              <button
                type="submit"
                disabled={loading}
                className="btn-primary w-full"
              >
                {loading ? t('auth.reset.submitting') : t('auth.reset.submit')}
              </button>
            </form>
          )}
        </div>
        <div className="flex justify-center">
          <LanguageSwitcher />
        </div>
      </div>
    </div>
  );
}
