import { useState } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import { forgotPassword } from '@/api/admin';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { AuthLayout } from '@/components/layout/AuthLayout';
import { TurnstileWidget } from '@/components/TurnstileWidget';
import { turnstileEnabled } from '@/lib/turnstile';
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
      if (turnstileEnabled() && !token) {
        resetTurnstileWidget();
        setError(t('auth.verificationSlow'));
        return;
      }
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
    <AuthLayout>
      <div className="bg-[var(--card)] border border-[var(--border)] rounded-xl shadow-sm p-6 sm:p-7">
        <div className="text-center mb-6">
          <BrandLogo variant="lockup-auto" priority className="h-10 w-auto mx-auto mb-3 rounded-lg" />
          <h1 className="text-xl font-semibold text-[var(--foreground)]">{t('auth.forgot.title')}</h1>
          <p className="text-sm text-[var(--muted-foreground)] mt-1">
            {t('auth.forgot.subtitle')}
          </p>
        </div>

        {submitted ? (
          <div className="text-center space-y-4">
            <div className="w-12 h-12 bg-emerald-50 dark:bg-emerald-500/15 border border-emerald-200 dark:border-emerald-500/30 rounded-lg flex items-center justify-center mx-auto">
              <svg className="w-6 h-6 text-emerald-600 dark:text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <div>
              <p className="text-sm text-[var(--muted-foreground)]">
                {t('auth.forgot.sentBody')}
              </p>
            </div>
            <Link
              to="/login"
              className="inline-flex min-h-11 items-center justify-center px-3 text-sm text-[var(--accent-1)] font-medium hover:underline rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
            >
              {t('auth.backToLogin')}
            </Link>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="space-y-5">
            {error && (
              <div role="alert" className="bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 text-red-700 dark:text-red-300 text-sm px-3.5 py-2.5 rounded-lg">
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
                action="forgot_password"
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
                  {t('auth.submitting')}
                </>
              ) : (
                t('auth.forgot.submit')
              )}
            </button>

            <div className="text-center">
              <Link
                to="/login"
                className="inline-flex min-h-11 items-center justify-center px-3 text-sm text-[var(--muted-foreground)] hover:text-[var(--foreground)] rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
              >
                {t('auth.backToLogin')}
              </Link>
            </div>
          </form>
        )}
      </div>
    </AuthLayout>
  );
}
