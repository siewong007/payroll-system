# Login Page Simplification — Design

Date: 2026-09-13
Status: approved-by-user (inline-expand chosen via options question)
Scope: `frontend/src/pages/auth/Login.tsx` only. No backend changes.

## Problem

The current login card shows every credential type at once: a Turnstile
widget pinned to the top, a Google button, a three-button grid (Passkey /
Authenticator / Recovery code), a divider, then the email+password form.
The user finds it messy and wants Google + email/password as the only
always-visible methods.

## Target layout (default view)

```
[BrandLogo]
Malaysian Payroll System

[ G  Continue with Google ]

──────── Other ways to sign in ────────

Email    [........................]
Password [........................] 👁
[ Cloudflare Turnstile ]
[            Sign In            ]

Forgot password?   ·   More sign-in options ⌄
```

1. **Google stays primary** — unchanged button, unchanged handler.
2. **Divider relabeled** from "or sign in with email" to "Other ways to
   sign in" (the user's own label for the email/password block).
3. **Password visibility toggle** — eye icon button inside the password
   input flipping `type` between `password` and `text`. This is a
   standard, NIST-endorsed pattern (reduces lockouts; the credential is
   unchanged), not a security issue. `aria-label`/`aria-pressed` for
   a11y; `autoComplete="current-password"` kept.
4. **Turnstile moves** below the password field, directly above Sign In.
   It mints on mount regardless of position, so Google sign-in above it
   still gets a token via the existing `waitForTurnstileToken` waiter.
5. **"More sign-in options" link** sits in the footer row next to
   "Forgot password?". Clicking expands an inline section (the three
   existing method buttons: Passkey — only when `webauthnSupported`,
   Authenticator, Recovery code) with a chevron that rotates; clicking
   again collapses. Google and the email form remain visible while
   expanded.

## Turnstile mounting change

Today one `<TurnstileWidget>` is mounted *outside* the method-picker
conditional so its token survives the swap to the code-login view. After
this redesign the widget lives *inside* each form, directly above its
submit button:

- Default view: below password, above Sign In.
- Code-login view (`codeMethod` set): below the code input, above its
  Sign In.

Because the two branches are mutually exclusive, only one widget exists
at a time; remounting on view swap re-mints a token, which the existing
`waitForTurnstileToken`/`takeTurnstileToken` machinery already tolerates
(5s waiter). The "keep it mounted across the picker" comment is updated
to reflect the per-view mount.

The `hasPasskey` email probe (`checkPasskey`) is unchanged — it consumes
and resets whichever widget is mounted.

## Behavior details

- "More sign-in options" state is a plain `useState<boolean>`;
  collapsed by default; resets are not needed on view swap (the code
  view replaces the whole default block anyway).
- Authenticator / Recovery code buttons still call
  `setCodeMethod('totp' | 'backup')` — the existing code form is
  untouched apart from gaining its own Turnstile mount.
- Passkey button keeps both flows (email-targeted when `hasPasskey`,
  discoverable otherwise).
- Error alert, loading states, `mfaToken`/`TwoFactorPrompt` swap, and
  `?redirect=` handling are all unchanged.
- No new dependencies. `Eye`/`EyeOff` from `lucide-react`;
  `AnimatePresence`/`motion` expand animation matching the app's
  existing framer-motion grammar (kept subtle — height/opacity).

## Testing

Update `CodeLogin.test.tsx`: the three method buttons now sit behind the
"More sign-in options" disclosure, so tests click it first; the "Other
ways to sign in" text becomes the divider and remains asserted. Add a
test for the disclosure toggle and for the password visibility toggle
(`type` flips, `aria-pressed` flips). `Turnstile.test.tsx` is
unaffected (widget internals unchanged).

Verification: `bun run test`, `bun run lint`, `bun run typecheck`,
`bun run build` from `frontend/`. Then `./scripts/codegraph update .`.

## Explicitly out of scope

- No backend changes; Turnstile enforcement is unchanged.
- The `/forgot-password` and `/reset-password` pages keep their own
  widget placement (they're single-purpose forms, already ordered
  correctly).
- `TwoFactorPrompt`, `ChangePasswordCard`, and other password fields do
  not gain the eye toggle in this change (could be a follow-up).
- Employee portal and kiosk routes are untouched.
