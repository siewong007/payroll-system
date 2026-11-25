import { useState } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { forgotPassword } from '@/api/admin';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { LanguageSwitcher } from '@/components/ui/LanguageSwitcher';
import { TurnstileWidget } from '@/components/TurnstileWidget';
import { useTurnstileWaiter } from '@/lib/useTurnstileWaiter';
import { getErrorMessage } from '@/lib/utils';

export function ForgotPassword() {
  const { t } = useTranslation();
  const [email, setEmail] = useState('');
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const {
    widgetRef: turnstileRef,
    waitForToken: waitForTurnstileToken,
    handleVerify: handleTurnstileVerify,
    handleExpire: handleTurnstileExpire,
    handleError: handleTurnstileError,
    resetWidget: resetTurnstileWidget,
  } = useTurnstileWaiter();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      const token = await waitForTurnstileToken();
      await forgotPassword(email, token);
      setSubmitted(true);
    } catch (err: unknown) {
      resetTurnstileWidget();
      setError(getErrorMessage(err, t('auth.forgot.failed')));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-100 dark:bg-slate-950 py-6">
      <div className="animate-page-enter w-full max-w-md px-4 space-y-5">
        <div className="bg-white dark:bg-slate-900 dark:border dark:border-slate-800 rounded-2xl shadow p-6 sm:p-8">
          <div className="text-center mb-8">
            <BrandLogo variant="lockup-dark" priority className="h-10 w-auto mx-auto mb-4 rounded-xl" />
            <h1 className="text-xl font-semibold text-gray-900 dark:text-slate-100">{t('auth.forgot.title')}</h1>
            <p className="text-sm text-gray-500 dark:text-slate-400 mt-1">
              {t('auth.forgot.subtitle')}
            </p>
          </div>

          {submitted ? (
            <div className="text-center space-y-4">
              <div className="w-16 h-16 bg-green-50 dark:bg-emerald-500/15 rounded-full flex items-center justify-center mx-auto">
                <svg className="w-8 h-8 text-green-500 dark:text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
              </div>
              <div>
                <p className="text-sm text-gray-600 dark:text-slate-300">
                  {t('auth.forgot.sentBody')}
                </p>
              </div>
              <Link
                to="/login"
                className="inline-block text-sm text-black dark:text-indigo-400 font-medium hover:underline"
              >
                {t('auth.backToLogin')}
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
                <label className="form-label" htmlFor="forgot-email">{t('auth.email')}</label>
                <input
                  id="forgot-email"
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  className="form-input"
                  placeholder={t('auth.emailPlaceholder')}
                  autoComplete="email"
                  required
                />
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
                className="btn-primary w-full"
              >
                {loading ? t('auth.submitting') : t('auth.forgot.submit')}
              </button>

              <div className="text-center">
                <Link to="/login" className="text-sm text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200">
                  {t('auth.backToLogin')}
                </Link>
              </div>
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
