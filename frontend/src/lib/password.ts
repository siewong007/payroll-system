/**
 * Client-side mirror of the backend's `auth_service::validate_password_strength`.
 *
 * Kept deliberately in one place so the hint text and the check cannot drift
 * apart from each other or from the server — the previous form advertised
 * "Minimum 6 characters" while the API rejected anything under 10, so the first
 * thing a new administrator saw was a server error.
 */

export const PASSWORD_MIN_LENGTH = 10;

export const PASSWORD_POLICY_HINT =
  'At least 10 characters, with an uppercase letter, a lowercase letter, and a digit';

/** Returns an error message, or `null` when the password satisfies the policy. */
export function validatePassword(password: string): string | null {
  if (password.length < PASSWORD_MIN_LENGTH) {
    return `Password must be at least ${PASSWORD_MIN_LENGTH} characters`;
  }
  if (!/[A-Z]/.test(password) || !/[a-z]/.test(password) || !/[0-9]/.test(password)) {
    return 'Password must contain uppercase, lowercase, and a digit';
  }
  return null;
}
