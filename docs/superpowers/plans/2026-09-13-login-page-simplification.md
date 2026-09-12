# Login Page Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Declutter the login card to Google + email/password, move Turnstile above Sign In, hide passkey/authenticator/recovery behind an inline-expanding link, and add a password show/hide toggle.

**Architecture:** Single-file frontend change in `frontend/src/pages/auth/Login.tsx`. Turnstile moves from one shared top-mounted widget to a per-view mount inside each form (default + code-login), directly above the submit button. Spec: `docs/superpowers/specs/2026-09-13-login-page-simplification-design.md`.

**Tech Stack:** React 19, Tailwind v4, framer-motion, lucide-react, Vitest + Testing Library.

## Global Constraints

- No backend changes; Turnstile enforcement unchanged.
- No new dependencies. Icons from `lucide-react` (`Eye`, `EyeOff`, `ChevronDown`).
- Turnstile tokens remain single-use; keep `takeTurnstileToken` / `waitForTurnstileToken` / `turnstileRef` machinery exactly as is.
- Only one `<TurnstileWidget>` may be mounted at a time (the two mount points live in mutually exclusive branches).
- Keep `autoComplete="current-password"` on the password input when adding the toggle.
- Run all commands from `frontend/`.

---

### Task 1: Update and extend Login tests (failing)

**Files:**
- Modify: `frontend/src/tests/CodeLogin.test.tsx`
- Modify: `frontend/src/tests/WebAuthn.test.ts`

**Interfaces:**
- Produces: tests expecting the new disclosure (`More sign-in options` button, `aria-expanded`), the eye toggle (`Show password` / `Hide password` buttons flipping input `type`), the relabeled divider (`Other ways to sign in` visible by default), and method buttons hidden until expanded.

- [ ] **Step 1: Rewrite `CodeLogin.test.tsx`**

Replace the file with:

```tsx
import { beforeEach, describe, expect, it, vi, afterEach } from 'vitest';
import { createElement, type ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { AuthProvider } from '@/context/AuthProvider';
import { Login } from '@/pages/auth/Login';

const apiMocks = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
  put: vi.fn(),
  setAccessToken: vi.fn(),
}));

vi.mock('@/api/client', () => ({
  default: { get: apiMocks.get, post: apiMocks.post, put: apiMocks.put },
  setAccessToken: apiMocks.setAccessToken,
}));

const sessionUser = {
  id: 'user-1',
  email: 'employee@example.com',
  full_name: 'Employee User',
  roles: ['employee'],
  company_id: 'company-1',
  employee_id: 'emp-1',
};

function renderLogin() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const tree = (children: ReactNode) =>
    createElement(
      QueryClientProvider,
      { client: queryClient },
      createElement(AuthProvider, null, createElement(MemoryRouter, null, children)),
    );
  return render(tree(createElement(Login)));
}

async function expandMoreOptions(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: /more sign-in options/i }));
}

describe('code sign-in (authenticator / recovery code)', () => {
  beforeEach(() => {
    // .env.local supplies a real site key; a Turnstile-enabled test run would
    // block every submit behind a widget jsdom cannot mint.
    vi.stubEnv('VITE_TURNSTILE_SITE_KEY', '');

    apiMocks.setAccessToken.mockReset();
    apiMocks.get.mockReset().mockResolvedValue({ data: [] });
    apiMocks.put.mockReset().mockResolvedValue({ data: {} });
    apiMocks.post.mockReset().mockImplementation((url: string) => {
      switch (url) {
        case '/auth/refresh':
          return Promise.reject(new Error('No session'));
        case '/auth/login/code':
          return Promise.resolve({ data: { token: 'session-token', user: sessionUser } });
        default:
          return Promise.resolve({ data: {} });
      }
    });
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('labels the email form as Other ways to sign in and hides alternates by default', async () => {
    renderLogin();

    expect(await screen.findByText('Other ways to sign in')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /authenticator/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /recovery code/i })).not.toBeInTheDocument();
  });

  it('expands and collapses the extra sign-in methods via the link', async () => {
    const user = userEvent.setup();
    renderLogin();

    const toggle = await screen.findByRole('button', { name: /more sign-in options/i });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    await user.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(await screen.findByRole('button', { name: /authenticator/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /recovery code/i })).toBeInTheDocument();

    await user.click(toggle);
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: /authenticator/i })).not.toBeInTheDocument(),
    );
  });

  it('toggles password visibility', async () => {
    const user = userEvent.setup();
    renderLogin();

    const password = await screen.findByPlaceholderText('Enter your password');
    expect(password).toHaveAttribute('type', 'password');
    await user.click(screen.getByRole('button', { name: /show password/i }));
    expect(password).toHaveAttribute('type', 'text');
    await user.click(screen.getByRole('button', { name: /hide password/i }));
    expect(password).toHaveAttribute('type', 'password');
  });

  it('shows an email + authenticator code form and submits both to /auth/login/code', async () => {
    const user = userEvent.setup();
    renderLogin();

    await expandMoreOptions(user);
    await user.click(await screen.findByRole('button', { name: /^authenticator$/i }));
    await user.type(screen.getByPlaceholderText('Enter your email'), 'employee@example.com');
    await user.type(screen.getByPlaceholderText('6-digit code'), '123456');
    await user.click(screen.getByRole('button', { name: /^sign in$/i }));

    await waitFor(() =>
      expect(apiMocks.post).toHaveBeenCalledWith('/auth/login/code', {
        email: 'employee@example.com',
        code: '123456',
        turnstile_token: undefined,
      }),
    );
    // A successful code login is a complete session — the user is stored.
    await waitFor(() =>
      expect(JSON.parse(localStorage.getItem('user') ?? '{}')).toMatchObject({
        email: 'employee@example.com',
      }),
    );
  });

  it('switches the code field label and placeholder for recovery codes', async () => {
    const user = userEvent.setup();
    renderLogin();

    await expandMoreOptions(user);
    await user.click(await screen.findByRole('button', { name: /recovery code/i }));

    expect(await screen.findByText('Recovery code')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('e.g. 1A2B-3C4D')).toBeInTheDocument();
  });

  it('surfaces the server rejection instead of a generic message', async () => {
    apiMocks.post.mockImplementation((url: string) => {
      if (url === '/auth/refresh') return Promise.reject(new Error('No session'));
      if (url === '/auth/login/code') {
        return Promise.reject({
          response: { status: 401, data: { error: 'Invalid email or code' } },
        });
      }
      return Promise.resolve({ data: {} });
    });

    const user = userEvent.setup();
    renderLogin();

    await expandMoreOptions(user);
    await user.click(await screen.findByRole('button', { name: /^authenticator$/i }));
    await user.type(screen.getByPlaceholderText('Enter your email'), 'employee@example.com');
    await user.type(screen.getByPlaceholderText('6-digit code'), '000000');
    await user.click(screen.getByRole('button', { name: /^sign in$/i }));

    expect(await screen.findByText('Invalid email or code')).toBeInTheDocument();
  });

  it('returns to the default view via the back link', async () => {
    const user = userEvent.setup();
    renderLogin();

    await expandMoreOptions(user);
    await user.click(await screen.findByRole('button', { name: /^authenticator$/i }));
    await user.click(screen.getByRole('button', { name: /back to all sign-in options/i }));

    expect(await screen.findByPlaceholderText('Enter your password')).toBeInTheDocument();
    expect(screen.getByText('Other ways to sign in')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Update the two passkey call-site tests in `WebAuthn.test.ts`**

The `describe('passkey login call sites')` block clicks "Sign in with Passkey" twice; that button now lives behind the disclosure. In the discoverable test (~line 292), insert before the passkey click:

```tsx
    await user.click(await screen.findByRole('button', { name: /more sign-in options/i }));
    await user.click(await screen.findByRole('button', { name: /sign in with passkey/i }));
```

In the email-flow test (~line 329), after the `act` flush and before the passkey click:

```tsx
    await user.click(screen.getByRole('button', { name: /more sign-in options/i }));
    await user.click(screen.getByRole('button', { name: /sign in with passkey/i }));
```

- [ ] **Step 3: Run the file and confirm the new expectations fail**

Run: `cd frontend && bun run test -- CodeLogin WebAuthn`
Expected: FAIL — no "More sign-in options" button, no "Show password" button, alternates visible without disclosure.

---

### Task 2: Rewrite Login.tsx to the simplified layout

**Files:**
- Modify: `frontend/src/pages/auth/Login.tsx`

**Interfaces:**
- Consumes: existing `TurnstileWidget`/`TurnstileWidgetRef`, `turnstileEnabled`, `codeLogin`, passkey APIs — all unchanged.
- Produces: same exported `Login` component; new internal state `showPassword`, `showMoreOptions`.

- [ ] **Step 1: Update imports and state**

In `frontend/src/pages/auth/Login.tsx`:

- Import line 4: `import { motion } from 'framer-motion';` → `import { AnimatePresence, motion } from 'framer-motion';`
- Import line 6: `import { Fingerprint, LifeBuoy, Smartphone } from 'lucide-react';` → `import { ChevronDown, Eye, EyeOff, Fingerprint, LifeBuoy, Smartphone } from 'lucide-react';`
- After `const [password, setPassword] = useState('');` add:

```tsx
  const [showPassword, setShowPassword] = useState(false);
  const [showMoreOptions, setShowMoreOptions] = useState(false);
```

- [ ] **Step 2: Move the widget out of the shared top position**

Delete the top-mounted `<TurnstileWidget ... />` block (old lines ~228-243) **and** the comment above it about staying mounted across the picker. Each form mounts its own instance (Step 4 and Step 5). Extract the shared props into a JSX constant directly above the `return (`:

```tsx
  // One widget per view, mounted directly above each form's submit button.
  // The branches below are mutually exclusive, so only one instance exists
  // at a time; a view swap remounts and re-mints, which the waiter tolerates.
  const turnstile = (
    <TurnstileWidget
      ref={turnstileRef}
      onVerify={(t) => {
        turnstileToken.current = t;
        turnstileWaiter.current?.();
      }}
      onExpire={() => {
        turnstileToken.current = undefined;
      }}
      onError={() => {
        turnstileToken.current = undefined;
      }}
    />
  );
```

- [ ] **Step 3: Reorder the default view**

In the `codeMethod ? ... : (...)` false-branch, restructure to: Google button → divider → email/password form (with eye toggle, per-view widget, footer links) → expandable methods section.

Replace the entire false-branch fragment (old lines ~303-405) with:

```tsx
                <>
                  {/* Third-party identity stays on its own */}
                  {googleProvider && (
                    <button
                      type="button"
                      onClick={handleGoogleLogin}
                      disabled={googleLoading}
                      className="w-full flex items-center justify-center gap-3 py-2.5 px-4 bg-white border border-gray-200 rounded-xl text-sm font-medium text-gray-700 hover:border-gray-300 hover:shadow-md hover:-translate-y-px disabled:opacity-50 transition-all"
                    >
                      <GoogleIcon />
                      {googleLoading ? 'Verifying...' : 'Continue with Google'}
                    </button>
                  )}

                  <div className="flex items-center gap-3 my-6">
                    <div className="h-px flex-1 bg-gray-200" />
                    <span className="text-xs text-gray-400">Other ways to sign in</span>
                    <div className="h-px flex-1 bg-gray-200" />
                  </div>

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
                      <div className="relative">
                        <input
                          type={showPassword ? 'text' : 'password'}
                          value={password}
                          onChange={(e) => setPassword(e.target.value)}
                          className="form-input pr-11"
                          placeholder="Enter your password"
                          autoComplete="current-password"
                          required
                        />
                        <button
                          type="button"
                          onClick={() => setShowPassword((v) => !v)}
                          aria-label={showPassword ? 'Hide password' : 'Show password'}
                          aria-pressed={showPassword}
                          className="absolute inset-y-0 right-0 flex items-center px-3 text-gray-400 hover:text-gray-600 transition-colors"
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
                      disabled={loading}
                      className="w-full bg-gradient-to-r from-slate-900 to-slate-700 text-white py-2.5 rounded-xl font-semibold shadow-lg hover:shadow-[0_10px_30px_-8px_rgba(99,102,241,0.5),0_10px_30px_-8px_rgba(20,184,166,0.4)] hover:-translate-y-px active:translate-y-0 disabled:opacity-50 disabled:shadow-none transition-all"
                    >
                      {loading ? 'Signing in...' : 'Sign In'}
                    </button>

                    <div className="flex items-center justify-center gap-3 text-sm">
                      <Link to="/forgot-password" className="text-gray-500 hover:text-gray-700">
                        Forgot password?
                      </Link>
                      <span className="text-gray-300">·</span>
                      <button
                        type="button"
                        onClick={() => setShowMoreOptions((v) => !v)}
                        aria-expanded={showMoreOptions}
                        className="inline-flex items-center gap-1 text-gray-500 hover:text-gray-700"
                      >
                        More sign-in options
                        <ChevronDown
                          className={`w-4 h-4 transition-transform ${showMoreOptions ? 'rotate-180' : ''}`}
                        />
                      </button>
                    </div>
                  </form>

                  <AnimatePresence initial={false}>
                    {showMoreOptions && (
                      <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        transition={{ duration: 0.25, ease: [0.22, 1, 0.36, 1] }}
                        className="overflow-hidden"
                      >
                        <div
                          className={`grid gap-2 pt-4 ${webauthnSupported ? 'grid-cols-3' : 'grid-cols-2'}`}
                        >
                          {webauthnSupported && (
                            <button
                              type="button"
                              onClick={handlePasskeyLogin}
                              disabled={passkeyLoading}
                              className="flex flex-col items-center gap-1.5 py-2.5 px-2 bg-white border border-gray-200 rounded-xl text-xs font-medium text-gray-600 hover:border-gray-300 hover:shadow-sm disabled:opacity-50 transition-all"
                            >
                              <Fingerprint className="w-5 h-5" />
                              {passkeyLoading ? 'Verifying...' : 'Sign in with Passkey'}
                            </button>
                          )}
                          <button
                            type="button"
                            onClick={() => setCodeMethod('totp')}
                            className="flex flex-col items-center gap-1.5 py-2.5 px-2 bg-white border border-gray-200 rounded-xl text-xs font-medium text-gray-600 hover:border-gray-300 hover:shadow-sm transition-all"
                          >
                            <Smartphone className="w-5 h-5" />
                            Authenticator
                          </button>
                          <button
                            type="button"
                            onClick={() => setCodeMethod('backup')}
                            className="flex flex-col items-center gap-1.5 py-2.5 px-2 bg-white border border-gray-200 rounded-xl text-xs font-medium text-gray-600 hover:border-gray-300 hover:shadow-sm transition-all"
                          >
                            <LifeBuoy className="w-5 h-5" />
                            Recovery code
                          </button>
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </>
```

- [ ] **Step 4: Give the code-login view its own widget**

In the `codeMethod` form (true-branch), insert `{turnstile}` between the code input `<div>` and the submit `<button>`:

```tsx
                  <div className="flex justify-center">{turnstile}</div>

                  <button
                    type="submit"
                    disabled={loading}
                    ...
                  >
```

- [ ] **Step 5: Run the tests — all pass**

Run: `cd frontend && bun run test -- CodeLogin WebAuthn`
Expected: PASS (9 tests)

- [ ] **Step 6: Commit**

```bash
git add frontend/src/pages/auth/Login.tsx frontend/src/tests/CodeLogin.test.tsx frontend/src/tests/WebAuthn.test.ts
git commit -m "feat(frontend): simplify login page to Google + email/password"
```

---

### Task 3: Full verification + codegraph

**Files:** none modified

- [ ] **Step 1:** `cd frontend && bun run test` — full suite green
- [ ] **Step 2:** `cd frontend && bun run lint` — clean
- [ ] **Step 3:** `cd frontend && bun run typecheck` — clean
- [ ] **Step 4:** `cd frontend && bun run build` — succeeds
- [ ] **Step 5:** `./scripts/codegraph update .` — graph refreshed
- [ ] **Step 6:** `git status` clean; amend/commit any stragglers
