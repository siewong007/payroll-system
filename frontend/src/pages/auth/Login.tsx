import { useState, useEffect, useRef } from 'react';
import { useNavigate, Navigate, Link, useSearchParams } from 'react-router';
import type { User } from '@/types';
import { motion } from 'framer-motion';
import { useQuery } from '@tanstack/react-query';
import { Fingerprint } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { getErrorMessage, safeRedirectPath } from '@/lib/utils';
import { hasOnlyEmployeeRole } from '@/lib/roles';
import { checkPasskey, passkeyAuthBegin, passkeyAuthComplete, passkeyDiscoverableBegin, passkeyDiscoverableComplete } from '@/api/passkey';
import { getPasskeyCredential, isWebAuthnSupported } from '@/lib/webauthn';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { TwoFactorPrompt } from '@/components/TwoFactorPrompt';
import { GoogleIcon } from '@/components/ui/GoogleIcon';
import { getGoogleAuthorizeUrl, getOAuth2Providers } from '@/api/oauth2';
import { TurnstileWidget, type TurnstileWidgetRef } from '@/components/TurnstileWidget';
import { turnstileEnabled } from '@/lib/turnstile';

export function Login() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [passkeyLoading, setPasskeyLoading] = useState(false);
  const [hasPasskey, setHasPasskey] = useState(false);
  const [webauthnSupported] = useState(isWebAuthnSupported());
  const [mfaToken, setMfaToken] = useState<string | null>(null);
  const { login, setSession, user, isAuthenticated } = useAuth();
  const navigate = useNavigate();
  // The kiosk scan page sends unauthenticated scanners here with the scan URL
  // (including its QR token) in `?redirect=`; without honouring it they landed on
  // /portal and had to rescan within the token's 300s TTL.
  const [searchParams] = useSearchParams();
  const redirectTo = safeRedirectPath(searchParams.get('redirect'));

  // One widget serves every submit on this page (password, passkey, Google).
  // Tokens are single-use, so each send takes the current token, clears it,
  // and resets the widget to mint the next one.
  const turnstileRef = useRef<TurnstileWidgetRef>(null);
  const turnstileToken = useRef<string | undefined>(undefined);
  const takeTurnstileToken = () => {
    const token = turnstileToken.current;
    turnstileToken.current = undefined;
    turnstileRef.current?.reset();
    return token;
  };

  const { data: providers } = useQuery({
    queryKey: ['oauth2-providers'],
    queryFn: getOAuth2Providers,
    staleTime: 300_000,
  });

  const googleProvider = providers?.find((p) => p.provider === 'google' && p.enabled);

  // Check if email has passkeys when email changes
  useEffect(() => {
    if (!webauthnSupported || !email || !email.includes('@')) {
      setHasPasskey(false);
      return;
    }
    const timer = setTimeout(async () => {
      try {
        // Widget still minting — skip this probe; the next keystroke retries.
        if (turnstileEnabled() && !turnstileToken.current) return;
        const { has_passkey } = await checkPasskey(email, takeTurnstileToken());
        setHasPasskey(has_passkey);
      } catch {
        setHasPasskey(false);
      }
    }, 500);
    return () => clearTimeout(timer);
  }, [email, webauthnSupported]);

  if (isAuthenticated && user) {
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
      const result = await login(email, password, takeTurnstileToken());
      if (result.status === 'mfa_required') {
        setMfaToken(result.mfaToken);
      } else {
        goPostLogin(result.user);
      }
    } catch (err: unknown) {
      setError(getErrorMessage(err, 'Invalid email or password'));
    } finally {
      setLoading(false);
    }
  };

  const handlePasskeyLogin = async () => {
    setError('');
    setPasskeyLoading(true);
    try {
      let response;

      if (email && hasPasskey) {
        // Email-based flow: server sends allowCredentials for this user
        const { challenge_id, options } = await passkeyAuthBegin(email, takeTurnstileToken());
        const credential = await getPasskeyCredential(options.publicKey);
        response = await passkeyAuthComplete(challenge_id, credential);
      } else {
        // Discoverable flow: browser shows all available passkeys for this site
        const { challenge_id, options } = await passkeyDiscoverableBegin(takeTurnstileToken());
        const credential = await getPasskeyCredential(options.publicKey);
        response = await passkeyDiscoverableComplete(challenge_id, credential);
      }

      if ('requires_2fa' in response && response.requires_2fa) {
        setMfaToken(response.mfa_token);
      } else {
        setSession(response.token, response.user);
        navigate(redirectTo ?? (hasOnlyEmployeeRole(response.user) ? '/portal' : '/'));
      }
    } catch (err: unknown) {
      setError(getErrorMessage(err, 'Passkey authentication failed'));
    } finally {
      setPasskeyLoading(false);
    }
  };

  const handleGoogleLogin = async () => {
    try {
      window.location.href = await getGoogleAuthorizeUrl(undefined, takeTurnstileToken());
    } catch {
      setError('Google sign-in is not available');
    }
  };

  return (
    <div className="relative isolate min-h-screen flex items-center justify-center overflow-hidden bg-slate-950">
      {/* Aurora backdrop — indigo hints at the admin console, teal at the portal */}
      <div aria-hidden className="pointer-events-none absolute inset-0 -z-10">
        <div className="ambient-blob animate-float-a -top-32 -left-24 h-[30rem] w-[30rem] bg-indigo-600/30" />
        <div className="ambient-blob animate-float-b -bottom-40 -right-24 h-[32rem] w-[32rem] bg-teal-500/25" />
        <div className="ambient-blob animate-float-a top-1/3 left-1/2 h-72 w-72 bg-violet-600/20" />
        <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,transparent_35%,rgba(2,6,23,0.55)_100%)]" />
      </div>

      <motion.div
        className="w-full max-w-md px-4"
        initial={{ opacity: 0, y: 24, scale: 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.45, ease: [0.22, 1, 0.36, 1] }}
      >
        <div className="bg-white/90 backdrop-blur-2xl ring-1 ring-white/40 rounded-3xl shadow-2xl p-6 sm:p-8">
          {/* Logo */}
          <div className="text-center mb-8">
            <BrandLogo variant="lockup-dark" className="h-12 w-auto mx-auto mb-4" />
            <p className="text-sm text-gray-500 mt-1">Malaysian Payroll System</p>
          </div>

          {mfaToken ? (
            <TwoFactorPrompt
              mfaToken={mfaToken}
              onSuccess={goPostLogin}
              onBack={() => setMfaToken(null)}
            />
          ) : (
            <>
              {/* Social / Passkey Sign-In */}
              {(googleProvider || webauthnSupported) && (
                <>
                  <div className="space-y-2.5">
                    {googleProvider && (
                      <button
                        type="button"
                        onClick={handleGoogleLogin}
                        className="w-full flex items-center justify-center gap-3 py-2.5 px-4 bg-white border border-gray-200 rounded-xl text-sm font-medium text-gray-700 hover:border-gray-300 hover:shadow-md hover:-translate-y-px transition-all"
                      >
                        <GoogleIcon />
                        Continue with Google
                      </button>
                    )}
                    {webauthnSupported && (
                      <button
                        type="button"
                        onClick={handlePasskeyLogin}
                        disabled={passkeyLoading}
                        className="w-full flex items-center justify-center gap-3 py-2.5 px-4 bg-white border border-gray-200 rounded-xl text-sm font-medium text-gray-700 hover:border-gray-300 hover:shadow-md hover:-translate-y-px disabled:opacity-50 transition-all"
                      >
                        <Fingerprint className="w-5 h-5" />
                        {passkeyLoading ? 'Verifying...' : 'Sign in with Passkey'}
                      </button>
                    )}
                  </div>

                  <div className="flex items-center gap-3 my-6">
                    <div className="h-px flex-1 bg-gray-200" />
                    <span className="text-xs text-gray-400">or sign in with email</span>
                    <div className="h-px flex-1 bg-gray-200" />
                  </div>
                </>
              )}

              <form onSubmit={handleSubmit} className="space-y-5">
                {error && (
                  <div className="animate-fade-up bg-red-50 border border-red-100 text-red-600 text-sm px-4 py-3 rounded-xl">
                    {error}
                  </div>
                )}

                <div>
                  <label className="form-label">Email</label>
                  <input
                    type="email"
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    className="form-input"
                    placeholder="Enter your email"
                    required
                  />
                </div>

                <div>
                  <label className="form-label">Password</label>
                  <input
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    className="form-input"
                    placeholder="Enter your password"
                    required
                  />
                </div>

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

                <button
                  type="submit"
                  disabled={loading}
                  className="w-full bg-gradient-to-r from-slate-900 to-slate-700 text-white py-2.5 rounded-xl font-semibold shadow-lg hover:shadow-[0_10px_30px_-8px_rgba(99,102,241,0.5),0_10px_30px_-8px_rgba(20,184,166,0.4)] hover:-translate-y-px active:translate-y-0 disabled:opacity-50 disabled:shadow-none transition-all"
                >
                  {loading ? 'Signing in...' : 'Sign In'}
                </button>

                <div className="text-center">
                  <Link to="/forgot-password" className="text-sm text-gray-500 hover:text-gray-700">
                    Forgot password?
                  </Link>
                </div>
              </form>
            </>
          )}
        </div>

        <p className="mt-6 text-center text-xs text-slate-500">
          Secure payroll for Malaysian teams
        </p>
      </motion.div>
    </div>
  );
}
