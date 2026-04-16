import { useState, useEffect } from 'react';
import { useNavigate, Navigate, Link } from 'react-router-dom';
import { motion } from 'framer-motion';
import { useQuery } from '@tanstack/react-query';
import { Fingerprint } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import api from '@/api/client';
import { checkPasskey, passkeyAuthBegin, passkeyAuthComplete, passkeyDiscoverableBegin, passkeyDiscoverableComplete } from '@/api/passkey';
import { getPasskeyCredential, isWebAuthnSupported } from '@/lib/webauthn';

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
  const { login, setSession, user, isAuthenticated } = useAuth();
  const navigate = useNavigate();

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
    return <Navigate to={user.role === 'employee' ? '/portal' : '/'} replace />;
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      const loggedInUser = await login(email, password);
      if (loggedInUser.must_change_password) {
        navigate('/change-password');
      } else {
        navigate(loggedInUser.role === 'employee' ? '/portal' : '/');
      }
    } catch (err: any) {
      setError(err.response?.data?.error || 'Invalid email or password');
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
        const credential = await getPasskeyCredential(options);
        response = await passkeyAuthComplete(challenge_id, credential);
      } else {
        // Discoverable flow: browser shows all available passkeys for this site
        const { challenge_id, options } = await passkeyDiscoverableBegin();
        const credential = await getPasskeyCredential(options);
        response = await passkeyDiscoverableComplete(challenge_id, credential);
      }

      setSession(response.token, response.user);
      navigate(response.user.role === 'employee' ? '/portal' : '/');
    } catch (err: any) {
      const msg = err?.response?.data?.error || err?.message || 'Passkey authentication failed';
      setError(msg);
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
    <div className="min-h-screen flex items-center justify-center bg-gray-100">
      <motion.div
        className="w-full max-w-md px-4"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3 }}
      >
        <div className="bg-white rounded-2xl shadow p-6 sm:p-8">
          {/* Logo */}
          <div className="text-center mb-8">
            <div className="w-12 h-12 bg-black rounded-xl flex items-center justify-center mx-auto mb-4">
              <span className="text-white font-bold text-lg">P</span>
            </div>
            <h1 className="text-xl font-semibold text-gray-900">PayrollMY</h1>
            <p className="text-sm text-gray-400 mt-1">Malaysian Payroll System</p>
          </div>

          {/* Social / Passkey Sign-In */}
          {(googleProvider || webauthnSupported) && (
            <>
              <div className="space-y-2.5">
                {googleProvider && (
                  <button
                    type="button"
                    onClick={handleGoogleLogin}
                    className="w-full flex items-center justify-center gap-3 py-2.5 px-4 border border-gray-300 rounded-xl text-sm font-medium text-gray-700 hover:bg-gray-50 transition-all"
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
                    className="w-full flex items-center justify-center gap-3 py-2.5 px-4 border border-gray-300 rounded-xl text-sm font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-50 transition-all"
                  >
                    <Fingerprint className="w-5 h-5" />
                    {passkeyLoading ? 'Verifying...' : 'Sign in with Passkey'}
                  </button>
                )}
              </div>

              <div className="relative my-6">
                <div className="absolute inset-0 flex items-center">
                  <div className="w-full border-t border-gray-200" />
                </div>
                <div className="relative flex justify-center text-xs">
                  <span className="bg-white px-3 text-gray-400">or sign in with email</span>
                </div>
              </div>
            </>
          )}

          <form onSubmit={handleSubmit} className="space-y-5">
            {error && (
              <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-xl">
                {error}
              </div>
            )}

            <div>
              <label className="form-label">Email</label>
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                className="border p-2.5 rounded-lg w-full text-sm outline-none focus:border-black transition-colors"
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
                className="border p-2.5 rounded-lg w-full text-sm outline-none focus:border-black transition-colors"
                placeholder="Enter your password"
                required
              />
            </div>

            <button
              type="submit"
              disabled={loading}
              className="w-full bg-black text-white py-2.5 rounded-xl font-semibold hover:bg-gray-800 disabled:opacity-50 transition-all"
            >
              {loading ? 'Signing in...' : 'Sign In'}
            </button>

            <div className="text-center">
              <Link to="/forgot-password" className="text-sm text-gray-500 hover:text-gray-700">
                Forgot password?
              </Link>
            </div>
          </form>

        </div>
      </motion.div>
    </div>
  );
}
