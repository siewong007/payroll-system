import { useState, useEffect } from 'react';
import { useNavigate, Navigate, Link, useSearchParams } from 'react-router';
import type { User } from '@/types';
import { useQuery } from '@tanstack/react-query';
import { ChevronDown, Eye, EyeOff, Fingerprint, LifeBuoy, ScanLine, Smartphone } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { usePwa } from '@/context/PwaContext';
import { useTranslation } from 'react-i18next';
import { LanguageSwitcher } from '@/components/ui/LanguageSwitcher';
import { getErrorMessage, safeRedirectPath } from '@/lib/utils';
import { hasOnlyEmployeeRole } from '@/lib/roles';
import { checkPasskey, passkeyAuthBegin, passkeyAuthComplete, passkeyDiscoverableBegin, passkeyDiscoverableComplete } from '@/api/passkey';
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
  const [hasPasskey, setHasPasskey] = useState(false);
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
    takeToken: takeTurnstileToken,
    waitForToken: waitForTurnstileToken,
    handleVerify: handleTurnstileVerify,
    handleExpire: handleTurnstileExpire,
    handleError: handleTurnstileError,
  } = useTurnstileWaiter();
  const anySubmitting = loading || passkeyLoading || googleLoading;

  const { data: providers, isPending: providersPending } = useQuery({
    queryKey: ['oauth2-providers'],
    queryFn: getOAuth2Providers,
    staleTime: 300_000,
  });

  const googleProvider = providers?.find((p) => p.provider === 'google' && p.enabled);

  // Check if email has passkeys when email changes (only when Turnstile is off,
  // because probing /auth/passkey/check steals the single-use Turnstile token).
  useEffect(() => {
    if (!webauthnSupported || turnstileEnabled() || !email || !email.includes('@')) {
      setHasPasskey(false);
      return;
    }
    const timer = setTimeout(async () => {
      try {
        const { has_passkey } = await checkPasskey(email, takeTurnstileToken());
        setHasPasskey(has_passkey);
      } catch {
        setHasPasskey(false);
      }
    }, 500);
    return () => clearTimeout(timer);
  }, [email, takeTurnstileToken, webauthnSupported]);

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
      let response;

      if (email && hasPasskey) {
        // Email-based flow: server sends allowCredentials for this user
        const { challenge_id, options } = await passkeyAuthBegin(email, token);
        const credential = await getPasskeyCredential(options.publicKey);
        response = await passkeyAuthComplete(challenge_id, credential);
      } else {
        // Discoverable flow: browser shows all available passkeys for this site
        const { challenge_id, options } = await passkeyDiscoverableBegin(token);
        const credential = await getPasskeyCredential(options.publicKey);
        response = await passkeyDiscoverableComplete(challenge_id, credential);
      }

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
      ref={turnstileRef}
      onVerify={handleTurnstileVerify}
      onExpire={handleTurnstileExpire}
      onError={handleTurnstileError}
    />
  );

  const isQrScanRedirect = Boolean(redirectTo?.startsWith('/attendance/scan'));

  return (
    <div className="relative isolate min-h-screen flex items-center justify-center overflow-hidden bg-slate-950 py-6">
      {/* Aurora backdrop — indigo hints at the admin console, teal at the portal */}
      <div aria-hidden className="aurora-login pointer-events-none absolute inset-0 -z-10" />

      <div className="animate-page-enter w-full max-w-md px-4">
        <div className="bg-white/95 dark:bg-slate-900/95 ring-1 ring-white/40 dark:ring-slate-800 rounded-3xl shadow-2xl p-6 sm:p-8">
          {/* Logo */}
          <div className="text-center mb-6">
            <BrandLogo variant="lockup-dark" priority className="h-12 w-auto mx-auto mb-4 rounded-xl" />
            <p className="text-sm text-gray-500 dark:text-slate-400 mt-1">{t('auth.tagline')}</p>
          </div>

          {isQrScanRedirect && (
            <div className="mb-5 flex items-start gap-3 rounded-2xl border border-teal-200 dark:border-teal-500/30 bg-teal-50 dark:bg-teal-500/10 p-3.5 text-left">
              <ScanLine className="w-5 h-5 text-teal-600 dark:text-teal-300 shrink-0 mt-0.5" aria-hidden="true" />
              <div className="text-xs leading-relaxed">
                <p className="font-semibold text-teal-900 dark:text-teal-200">{t('auth.qrScanBannerTitle')}</p>
                <p className="text-teal-800/90 dark:text-teal-300/90 mt-0.5">{t('auth.qrScanBannerBody')}</p>
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
                      setCodeMethod(null);
                      setCodeValue('');
                      setError('');
                    }}
                    className="min-h-10 inline-flex items-center text-sm text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200"
                  >
                    ← {t('auth.backToOptions')}
                  </button>

                  {error && (
                    <div
                      id="login-error"
                      role="alert"
                      className="animate-fade-up bg-red-50 dark:bg-red-500/10 border border-red-100 dark:border-red-500/30 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-xl"
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
                    className="w-full min-h-11 bg-gradient-to-r from-slate-900 to-slate-700 dark:from-indigo-600 dark:to-indigo-500 text-white py-2.5 rounded-xl font-semibold shadow-lg hover:shadow-[0_10px_30px_-8px_rgba(99,102,241,0.5),0_10px_30px_-8px_rgba(20,184,166,0.4)] hover:-translate-y-px active:translate-y-0 disabled:opacity-50 disabled:shadow-none transition-all"
                  >
                    {loading ? t('auth.signingIn') : t('auth.signIn')}
                  </button>
                </form>
              ) : (
                <>
                  {/* Third-party identity stays on its own; reserve height while probing */}
                  {providersPending ? (
                    <div
                      className="h-[44px] w-full rounded-xl bg-gray-100 dark:bg-slate-800 animate-pulse"
                      aria-hidden="true"
                    />
                  ) : googleProvider ? (
                    <div className="space-y-2">
                      <button
                        type="button"
                        onClick={handleGoogleLogin}
                        disabled={anySubmitting}
                        className="w-full min-h-11 flex items-center justify-center gap-3 py-2.5 px-4 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl text-sm font-medium text-gray-700 dark:text-slate-100 hover:border-gray-300 dark:hover:border-slate-600 hover:shadow-md hover:-translate-y-px disabled:opacity-50 transition-all"
                      >
                        <GoogleIcon />
                        {googleLoading ? t('auth.verifying') : t('auth.continueGoogle')}
                      </button>
                      {isIos && isStandalone && (
                        <p className="text-xs text-center text-gray-500 dark:text-slate-400">
                          {t('auth.iosStandaloneGoogleNote')}
                        </p>
                      )}
                    </div>
                  ) : null}

                  <div className={`flex items-center gap-3 ${providersPending || googleProvider ? 'my-6' : 'mb-4 justify-center'}`}>
                    {(providersPending || googleProvider) && (
                      <div className="h-px flex-1 bg-gray-200 dark:bg-slate-700" />
                    )}
                    <span
                      className={`text-xs ${
                        providersPending || googleProvider
                          ? 'text-gray-400 dark:text-slate-400'
                          : 'font-medium text-gray-500 dark:text-slate-400'
                      }`}
                    >
                      {t('auth.otherWays')}
                    </span>
                    {(providersPending || googleProvider) && (
                      <div className="h-px flex-1 bg-gray-200 dark:bg-slate-700" />
                    )}
                  </div>

                  <form onSubmit={handleSubmit} className="space-y-5">
                    {error && (
                      <div
                        id="login-error"
                        role="alert"
                        className="animate-fade-up bg-red-50 dark:bg-red-500/10 border border-red-100 dark:border-red-500/30 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-xl"
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
                          className="absolute inset-y-0 right-0 flex items-center px-3 min-w-11 justify-center text-gray-400 dark:text-slate-400 hover:text-gray-600 dark:hover:text-slate-200 transition-colors"
                        >
                          {showPassword ? (
                            <EyeOff className="w-4 h-4" />
                          ) : (
                            <Eye className="w-4 h-4" />
                          )}
                        </button>
                      </div>
                    </div>

                    <div className="flex justify-center">{turnstile}</div>

                    <button
                      type="submit"
                      disabled={anySubmitting}
                      className="w-full min-h-11 bg-gradient-to-r from-slate-900 to-slate-700 dark:from-indigo-600 dark:to-indigo-500 text-white py-2.5 rounded-xl font-semibold shadow-lg hover:shadow-[0_10px_30px_-8px_rgba(99,102,241,0.5),0_10px_30px_-8px_rgba(20,184,166,0.4)] hover:-translate-y-px active:translate-y-0 disabled:opacity-50 disabled:shadow-none transition-all"
                    >
                      {loading ? t('auth.signingIn') : t('auth.signIn')}
                    </button>

                    <div className="flex flex-wrap items-center justify-center gap-3 text-sm">
                      <Link to="/forgot-password" className="text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200">
                        {t('auth.forgotPasswordLink')}
                      </Link>
                      <span className="text-gray-300 dark:text-slate-700">·</span>
                      <button
                        type="button"
                        onClick={() => setShowMoreOptions((v) => !v)}
                        aria-expanded={showMoreOptions}
                        className="inline-flex items-center gap-1 min-h-10 text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200"
                      >
                        {t('auth.moreOptions')}
                        <ChevronDown
                          className={`w-4 h-4 transition-transform ${showMoreOptions ? 'rotate-180' : ''}`}
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
                            className="min-h-11 flex flex-col items-center justify-center gap-1.5 py-2.5 px-2 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl text-xs font-medium text-gray-600 dark:text-slate-200 hover:border-gray-300 dark:hover:border-slate-600 hover:shadow-sm disabled:opacity-50 transition-all"
                          >
                            <Fingerprint className="w-5 h-5" />
                            {passkeyLoading ? t('auth.verifying') : t('auth.passkeySignIn')}
                          </button>
                        )}
                        <button
                          type="button"
                          onClick={() => {
                            setError('');
                            setCodeMethod('totp');
                          }}
                          disabled={anySubmitting}
                          className="min-h-11 flex flex-col items-center justify-center gap-1.5 py-2.5 px-2 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl text-xs font-medium text-gray-600 dark:text-slate-200 hover:border-gray-300 dark:hover:border-slate-600 hover:shadow-sm disabled:opacity-50 transition-all"
                        >
                          <Smartphone className="w-5 h-5" />
                          {t('auth.authenticator')}
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setError('');
                            setCodeMethod('backup');
                          }}
                          disabled={anySubmitting}
                          className="min-h-11 flex flex-col items-center justify-center gap-1.5 py-2.5 px-2 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl text-xs font-medium text-gray-600 dark:text-slate-200 hover:border-gray-300 dark:hover:border-slate-600 hover:shadow-sm disabled:opacity-50 transition-all"
                        >
                          <LifeBuoy className="w-5 h-5" />
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

        <div className="mt-6 space-y-2">
          <div className="flex items-center justify-center gap-4">
            <p className="text-center text-xs text-slate-500">
              {t('auth.footerTagline')}
            </p>
            <LanguageSwitcher dark />
          </div>
          <div className="flex flex-wrap items-center justify-center gap-3 text-xs text-slate-400">
            <Link to="/terms" className="hover:text-slate-200 transition-colors">
              {t('legal.links.terms')}
            </Link>
            <span className="text-slate-600">·</span>
            <Link to="/privacy" className="hover:text-slate-200 transition-colors">
              {t('legal.links.privacy')}
            </Link>
            <span className="text-slate-600">·</span>
            <Link to="/compliance" className="hover:text-slate-200 transition-colors">
              {t('legal.links.compliance')}
            </Link>
          </div>
        </div>
      </div>
    </div>
  );
}
