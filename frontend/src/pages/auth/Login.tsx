import { useState } from 'react';
import { useNavigate, Navigate, Link, useSearchParams } from 'react-router';
import type { User } from '@/types';
import { useQuery } from '@tanstack/react-query';
import { ChevronDown, Eye, EyeOff, Fingerprint, LifeBuoy, Loader2, ScanLine, Smartphone } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { usePwa } from '@/context/PwaContext';
import { useTranslation } from 'react-i18next';
import { AuthLayout } from '@/components/layout/AuthLayout';
import { getErrorMessage, safeRedirectPath } from '@/lib/utils';
import { hasOnlyEmployeeRole } from '@/lib/roles';
import { passkeyDiscoverableBegin, passkeyDiscoverableComplete } from '@/api/passkey';
import { codeLogin } from '@/api/totp';
import { getPasskeyCredential, isWebAuthnSupported } from '@/lib/webauthn';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { TwoFactorPrompt } from '@/components/TwoFactorPrompt';
import { GoogleIcon } from '@/components/ui/GoogleIcon';
import { getGoogleAuthorizeUrl, getOAuth2Providers } from '@/api/oauth2';
import { TurnstileWidget } from '@/components/TurnstileWidget';
import { turnstileEnabled } from '@/lib/turnstile';
import { useTurnstileWaiter } from '@/lib/useTurnstileWaiter';

export function Login() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [showMoreOptions, setShowMoreOptions] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [passkeyLoading, setPasskeyLoading] = useState(false);
  const [webauthnSupported] = useState(isWebAuthnSupported());
  const [mfaToken, setMfaToken] = useState<string | null>(null);
  const [codeMethod, setCodeMethod] = useState<'totp' | 'backup' | null>(null);
  const [codeValue, setCodeValue] = useState('');
  const [googleLoading, setGoogleLoading] = useState(false);
  const { login, setSession, user, isAuthenticated } = useAuth();
  const { isIos, isStandalone } = usePwa();
  const { t } = useTranslation();
  const navigate = useNavigate();
  // The kiosk scan page sends unauthenticated scanners here with the scan URL
  // (including its QR token) in `?redirect=`; without honouring it they landed on
  // /portal and had to rescan within the token's 300s TTL.
  const [searchParams] = useSearchParams();
  const redirectTo = safeRedirectPath(searchParams.get('redirect'));

  // One widget serves every submit on this page (password, passkey, Google).
  // Tokens are single-use, so each send takes the current token, clears it,
  // and resets the widget to mint the next one.
  const {
    widgetRef: turnstileRef,
    waitForToken: waitForTurnstileToken,
    handleVerify: handleTurnstileVerify,
    handleExpire: handleTurnstileExpire,
    handleError: handleTurnstileError,
    resetWidget: resetTurnstileWidget,
  } = useTurnstileWaiter();
  const anySubmitting = loading || passkeyLoading || googleLoading;

  const { data: providers, isPending: providersPending } = useQuery({
    queryKey: ['oauth2-providers'],
    queryFn: getOAuth2Providers,
    staleTime: 300_000,
  });

  const googleProvider = providers?.find((p) => p.provider === 'google' && p.enabled);

  if (isAuthenticated && user) {
    if (user.must_change_password) {
      return <Navigate to="/change-password" replace />;
    }
    return <Navigate to={redirectTo ?? (hasOnlyEmployeeRole(user) ? '/portal' : '/')} replace />;
  }

  const goPostLogin = (loggedInUser: User) => {
    if (loggedInUser.must_change_password) {
      navigate('/change-password');
    } else {
      navigate(redirectTo ?? (hasOnlyEmployeeRole(loggedInUser) ? '/portal' : '/'));
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      const token = await waitForTurnstileToken();
      if (turnstileEnabled() && !token) {
        setError(t('auth.verificationSlow'));
        return;
      }
      const result = await login(email, password, token);
      if (result.status === 'mfa_required') {
        setMfaToken(result.mfaToken);
      } else {
        goPostLogin(result.user);
      }
    } catch (err: unknown) {
      setError(getErrorMessage(err, t('auth.invalidCredentials')));
    } finally {
      setLoading(false);
    }
  };

  const handlePasskeyLogin = async () => {
    setError('');
    setPasskeyLoading(true);
    try {
      const token = await waitForTurnstileToken();
      if (turnstileEnabled() && !token) {
        setError(t('auth.verificationSlow'));
        return;
      }
      const { challenge_id, options } = await passkeyDiscoverableBegin(token);
      const credential = await getPasskeyCredential(options.publicKey);
      const response = await passkeyDiscoverableComplete(challenge_id, credential);

      if ('requires_2fa' in response && response.requires_2fa) {
        setMfaToken(response.mfa_token);
      } else {
        setSession(response.token, response.user);
        goPostLogin(response.user);
      }
    } catch (err: unknown) {
      setError(getErrorMessage(err, t('auth.passkeyFailed')));
    } finally {
      setPasskeyLoading(false);
    }
  };

  const handleGoogleLogin = async () => {
    setError('');
    setGoogleLoading(true);
    try {
      const token = await waitForTurnstileToken();
      if (turnstileEnabled() && !token) {
        setError(t('auth.verificationSlow'));
        return;
      }
      if (redirectTo) {
        sessionStorage.setItem('oauth2_login_redirect', redirectTo);
      } else {
        sessionStorage.removeItem('oauth2_login_redirect');
      }
      const authorizeUrl = await getGoogleAuthorizeUrl(undefined, token);
      window.location.assign(authorizeUrl);
    } catch {
      sessionStorage.removeItem('oauth2_login_redirect');
      setError(t('auth.googleUnavailable'));
    } finally {
      setGoogleLoading(false);
    }
  };

  // Passwordless sign-in with an authenticator/backup code: the code is both
  // factors at once, so a success returns a complete session (never an
  // mfa_required marker).
  const handleCodeLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      const token = await waitForTurnstileToken();
      if (turnstileEnabled() && !token) {
        setError(t('auth.verificationSlow'));
        return;
      }
      const data = await codeLogin(email, codeValue.trim(), token);
      setSession(data.token, data.user);
      goPostLogin(data.user);
    } catch (err: unknown) {
      setError(getErrorMessage(err, t('auth.invalidEmailOrCode')));
    } finally {
      setLoading(false);
    }
  };

  // One widget per view, mounted directly above each form's submit button.
  // The branches below are mutually exclusive, so only one instance exists
  // at a time; a view swap remounts and re-mints, which the waiter tolerates.
  const turnstile = (
    <TurnstileWidget
      key={codeMethod ? 'code_login' : 'login'}
      ref={turnstileRef}
      action={codeMethod ? 'code_login' : 'login'}
      onVerify={handleTurnstileVerify}
      onExpire={handleTurnstileExpire}
      onError={handleTurnstileError}
    />
  );

  const isQrScanRedirect = Boolean(redirectTo?.startsWith('/attendance/scan'));

  return (
    <AuthLayout variant="subtle">
      <div className="bg-[var(--card)] border border-[var(--border)] rounded-xl p-6 sm:p-7 text-[var(--foreground)] shadow-sm">
        {/* Logo */}
        <div className="text-center mb-6">
          <BrandLogo variant="lockup-auto" priority className="h-10 w-auto mx-auto mb-3 rounded-lg" />
          <p className="text-sm text-[var(--muted-foreground)] mt-1">{t('auth.tagline')}</p>
        </div>

        {isQrScanRedirect && (
          <div className="mb-5 flex items-start gap-3 rounded-lg border border-[var(--border)] bg-[var(--accent-soft)] p-3.5 text-left">
            <ScanLine className="w-5 h-5 text-[var(--accent-1)] shrink-0 mt-0.5" aria-hidden="true" />
            <div className="text-xs leading-relaxed">
              <p className="font-semibold text-[var(--foreground)]">{t('auth.qrScanBannerTitle')}</p>
              <p className="text-[var(--muted-foreground)] mt-0.5">{t('auth.qrScanBannerBody')}</p>
            </div>
          </div>
        )}

        {mfaToken ? (
          <TwoFactorPrompt
            mfaToken={mfaToken}
            onSuccess={goPostLogin}
            onBack={() => setMfaToken(null)}
          />
        ) : (
          <>
            {codeMethod ? (
              <form onSubmit={handleCodeLogin} className="space-y-5">
                <button
                  type="button"
                  onClick={() => {
                    resetTurnstileWidget();
                    setCodeMethod(null);
                    setCodeValue('');
                    setError('');
                  }}
                  className="min-h-11 inline-flex items-center text-sm text-[var(--muted-foreground)] hover:text-[var(--foreground)] rounded-lg px-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                >
                  ← {t('auth.backToOptions')}
                </button>

                {error && (
                  <div
                    id="login-error"
                    role="alert"
                    className="bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 text-red-700 dark:text-red-300 text-sm px-3.5 py-2.5 rounded-lg"
                  >
                    {error}
                  </div>
                )}

                <div>
                  <label className="form-label" htmlFor="code-email">{t('auth.email')}</label>
                  <input
                    id="code-email"
                    type="email"
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    className="form-input"
                    placeholder={t('auth.emailPlaceholder')}
                    aria-describedby={error ? 'login-error' : undefined}
                    required
                  />
                </div>

                <div>
                  <label className="form-label" htmlFor="code-value">
                    {codeMethod === 'totp' ? t('auth.authenticatorCode') : t('auth.recoveryCode')}
                  </label>
                  <input
                    id="code-value"
                    value={codeValue}
                    onChange={(e) => setCodeValue(e.target.value)}
                    className="form-input"
                    placeholder={
                      codeMethod === 'totp' ? t('auth.code6Placeholder') : t('auth.recoveryCodePlaceholder')
                    }
                    inputMode={codeMethod === 'totp' ? 'numeric' : 'text'}
                    autoComplete="one-time-code"
                    aria-describedby={error ? 'login-error' : undefined}
                    required
                  />
                </div>

                <div className="flex justify-center">{turnstile}</div>

                <button
                  type="submit"
                  disabled={anySubmitting}
                  className="btn-primary w-full min-h-11 justify-center rounded-lg"
                >
                  {loading ? (
                    <>
                      <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                      {t('auth.signingIn')}
                    </>
                  ) : (
                    t('auth.signIn')
                  )}
                </button>
              </form>
            ) : (
              <>
                {/* Third-party identity stays on its own; reserve height while probing */}
                {providersPending ? (
                  <div
                    className="h-11 w-full rounded-lg bg-[var(--surface-subtle)] animate-pulse"
                    aria-hidden="true"
                  />
                ) : googleProvider ? (
                  <div className="space-y-2">
                    <button
                      type="button"
                      onClick={handleGoogleLogin}
                      disabled={anySubmitting}
                      className="w-full min-h-11 flex items-center justify-center gap-3 py-2.5 px-4 bg-[var(--card)] border border-[var(--border)] rounded-lg text-sm font-medium text-[var(--foreground)] hover:bg-[var(--surface-subtle)] disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors"
                    >
                      {googleLoading ? (
                        <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                      ) : (
                        <GoogleIcon />
                      )}
                      {googleLoading ? t('auth.verifying') : t('auth.continueGoogle')}
                    </button>
                    {isIos && isStandalone && (
                      <p className="text-xs text-center text-gray-600 dark:text-slate-300">
                        {t('auth.iosStandaloneGoogleNote')}
                      </p>
                    )}
                  </div>
                ) : null}

                <div className={`flex items-center gap-3 ${providersPending || googleProvider ? 'my-6' : 'mb-4 justify-center'}`}>
                  {(providersPending || googleProvider) && (
                    <div className="h-px flex-1 bg-[var(--border)]" />
                  )}
                  <span
                    className={`text-xs ${
                      providersPending || googleProvider
                        ? 'text-[var(--muted-foreground)]'
                        : 'font-medium text-[var(--muted-foreground)]'
                    }`}
                  >
                    {t('auth.otherWays')}
                  </span>
                  {(providersPending || googleProvider) && (
                    <div className="h-px flex-1 bg-[var(--border)]" />
                  )}
                </div>

                <form onSubmit={handleSubmit} className="space-y-5">
                  {error && (
                    <div
                      id="login-error"
                      role="alert"
                      className="bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 text-red-700 dark:text-red-300 text-sm px-3.5 py-2.5 rounded-lg"
                    >
                      {error}
                    </div>
                  )}

                  <div>
                    <label className="form-label" htmlFor="login-email">{t('auth.email')}</label>
                    <input
                      id="login-email"
                      type="email"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      className="form-input"
                      placeholder={t('auth.emailPlaceholder')}
                      autoComplete="email"
                      aria-describedby={error ? 'login-error' : undefined}
                      required
                    />
                  </div>

                  <div>
                    <label className="form-label" htmlFor="login-password">{t('auth.password')}</label>
                    <div className="relative">
                      <input
                        id="login-password"
                        type={showPassword ? 'text' : 'password'}
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                        className="form-input pr-11"
                        placeholder={t('auth.passwordPlaceholder')}
                        autoComplete="current-password"
                        aria-describedby={error ? 'login-error' : undefined}
                        required
                      />
                      <button
                        type="button"
                        onClick={() => setShowPassword((v) => !v)}
                        aria-label={showPassword ? t('a11y.hidePassword') : t('a11y.showPassword')}
                        aria-pressed={showPassword}
                        className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] rounded-r-xl transition-colors"
                      >
                        {showPassword ? (
                          <EyeOff className="w-4 h-4" aria-hidden="true" />
                        ) : (
                          <Eye className="w-4 h-4" aria-hidden="true" />
                        )}
                      </button>
                    </div>
                  </div>

                  <div className="flex justify-center">{turnstile}</div>

                  <button
                    type="submit"
                    disabled={anySubmitting}
                    className="btn-primary w-full min-h-11 justify-center rounded-lg"
                  >
                    {loading ? (
                      <>
                        <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                        {t('auth.signingIn')}
                      </>
                    ) : (
                      t('auth.signIn')
                    )}
                  </button>

                  <div className="flex flex-wrap items-center justify-center gap-3 text-sm">
                    <Link
                      to="/forgot-password"
                      className="min-h-11 inline-flex items-center px-1 text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                    >
                      {t('auth.forgotPasswordLink')}
                    </Link>
                    <span aria-hidden="true" className="text-gray-300 dark:text-slate-700">·</span>
                    <button
                      type="button"
                      onClick={() => setShowMoreOptions((v) => !v)}
                      aria-expanded={showMoreOptions}
                      className="inline-flex items-center gap-1 min-h-11 px-1 text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                    >
                      {t('auth.moreOptions')}
                      <ChevronDown
                        className={`w-4 h-4 transition-transform ${showMoreOptions ? 'rotate-180' : ''}`}
                        aria-hidden="true"
                      />
                    </button>
                  </div>
                </form>

                {showMoreOptions && (
                  <div className="animate-fade-in overflow-hidden">
                    <div
                      className={`grid gap-2 pt-4 ${webauthnSupported ? 'grid-cols-1 sm:grid-cols-3' : 'grid-cols-1 sm:grid-cols-2'}`}
                    >
                      {webauthnSupported && (
                        <button
                          type="button"
                          onClick={handlePasskeyLogin}
                          disabled={anySubmitting}
                          className="min-h-11 flex flex-col items-center justify-center gap-1.5 py-2.5 px-2 bg-[var(--card)] border border-[var(--border)] rounded-lg text-xs font-medium text-[var(--foreground)] hover:bg-[var(--surface-subtle)] disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors"
                        >
                          {passkeyLoading ? (
                            <Loader2 className="w-5 h-5 animate-spin" aria-hidden="true" />
                          ) : (
                            <Fingerprint className="w-5 h-5" aria-hidden="true" />
                          )}
                          {passkeyLoading ? t('auth.verifying') : t('auth.passkeySignIn')}
                        </button>
                      )}
                      <button
                        type="button"
                        onClick={() => {
                          resetTurnstileWidget();
                          setError('');
                          setCodeMethod('totp');
                        }}
                        disabled={anySubmitting}
                        className="min-h-11 flex flex-col items-center justify-center gap-1.5 py-2.5 px-2 bg-[var(--card)] border border-[var(--border)] rounded-lg text-xs font-medium text-[var(--foreground)] hover:bg-[var(--surface-subtle)] disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors"
                      >
                        <Smartphone className="w-5 h-5" aria-hidden="true" />
                        {t('auth.authenticator')}
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          resetTurnstileWidget();
                          setError('');
                          setCodeMethod('backup');
                        }}
                        disabled={anySubmitting}
                        className="min-h-11 flex flex-col items-center justify-center gap-1.5 py-2.5 px-2 bg-[var(--card)] border border-[var(--border)] rounded-lg text-xs font-medium text-[var(--foreground)] hover:bg-[var(--surface-subtle)] disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors"
                      >
                        <LifeBuoy className="w-5 h-5" aria-hidden="true" />
                        {t('auth.recoveryCode')}
                      </button>
                    </div>
                  </div>
                )}
              </>
            )}
          </>
        )}
      </div>
    </AuthLayout>
  );
}
