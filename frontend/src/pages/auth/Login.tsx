import { useState, useEffect } from 'react';
import { useNavigate, Navigate, Link, useSearchParams } from 'react-router-dom';
import type { User } from '@/types';
import { motion } from 'framer-motion';
import { useQuery } from '@tanstack/react-query';
import { Fingerprint } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { getErrorMessage, safeRedirectPath } from '@/lib/utils';
import api from '@/api/client';
import { hasOnlyEmployeeRole } from '@/lib/roles';
import { checkPasskey, passkeyAuthBegin, passkeyAuthComplete, passkeyDiscoverableBegin, passkeyDiscoverableComplete } from '@/api/passkey';
import { getPasskeyCredential, isWebAuthnSupported } from '@/lib/webauthn';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { TwoFactorPrompt } from '@/components/TwoFactorPrompt';

function GoogleIcon() {
  return (
    <svg className="w-5 h-5" viewBox="0 0 24 24">
      <path
        d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 01-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z"
        fill="#4285F4"
      />
      <path
        d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
        fill="#34A853"
      />
      <path
        d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
        fill="#FBBC05"
      />
      <path
        d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
        fill="#EA4335"
      />
    </svg>
  );
}

interface OAuth2Provider {
  provider: string;
  enabled: boolean;
}

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

  const { data: providers } = useQuery({
    queryKey: ['oauth2-providers'],
    queryFn: () => api.get<OAuth2Provider[]>('/auth/oauth2/providers').then((r) => r.data),
    staleTime: 300_000,
    select: (data) => (Array.isArray(data) ? data : []),
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
        const { has_passkey } = await checkPasskey(email);
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
      const result = await login(email, password);
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
        const { challenge_id, options } = await passkeyAuthBegin(email);
        const credential = await getPasskeyCredential(options.publicKey);
        response = await passkeyAuthComplete(challenge_id, credential);
      } else {
        // Discoverable flow: browser shows all available passkeys for this site
        const { challenge_id, options } = await passkeyDiscoverableBegin();
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
      const { data } = await api.get<{ authorize_url: string }>('/auth/oauth2/google/authorize');
      window.location.href = data.authorize_url;
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
