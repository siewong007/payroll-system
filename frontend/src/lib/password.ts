/**
 * Client-side mirror of the backend's `auth_service::validate_password_strength`.
 *
 * Kept deliberately in one place so the hint text and the check cannot drift
 * apart from each other or from the server — the previous form advertised
 * "Minimum 6 characters" while the API rejected anything under 10, so the first
 * thing a new administrator saw was a server error.
 */

import i18n from '@/i18n';

export const PASSWORD_MIN_LENGTH = 10;

/** Policy hint, translated at call time so it follows the active locale. */
export function passwordPolicyHint(): string {
  return i18n.t('auth.passwordCard.policy', { count: PASSWORD_MIN_LENGTH });
}

/** Returns an error message, or `null` when the password satisfies the policy. */
export function validatePassword(password: string): string | null {
  if (password.length < PASSWORD_MIN_LENGTH) {
    return i18n.t('auth.passwordCard.tooShort', { count: PASSWORD_MIN_LENGTH });
  }
  if (!/[A-Z]/.test(password) || !/[a-z]/.test(password) || !/[0-9]/.test(password)) {
    return i18n.t('auth.passwordCard.complexity');
  }
  return null;
}
