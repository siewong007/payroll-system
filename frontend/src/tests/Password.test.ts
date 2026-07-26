import { describe, expect, it } from 'vitest';
import { PASSWORD_MIN_LENGTH, PASSWORD_POLICY_HINT, validatePassword } from '@/lib/password';

describe('password policy (mirrors backend validate_password_strength)', () => {
  it('accepts a compliant password', () => {
    expect(validatePassword('Str0ngPassword')).toBeNull();
  });

  it('rejects a password under the minimum length', () => {
    expect(validatePassword('Ab1cdef')).toMatch(/at least 10 characters/i);
  });

  it('rejects a password missing an uppercase letter', () => {
    expect(validatePassword('str0ngpassword')).toMatch(/uppercase/i);
  });

  it('rejects a password missing a lowercase letter', () => {
    expect(validatePassword('STR0NGPASSWORD')).toMatch(/lowercase/i);
  });

  it('rejects a password missing a digit', () => {
    expect(validatePassword('StrongPassword')).toMatch(/digit/i);
  });

  /**
   * Guards the specific drift this replaced: the form advertised "Minimum 6
   * characters" while the API required 10, so the first thing an administrator
   * saw when creating a user was a server-side rejection.
   */
  it('states the real minimum length in the hint shown to users', () => {
    expect(PASSWORD_MIN_LENGTH).toBe(10);
    expect(PASSWORD_POLICY_HINT).toContain('10 characters');
  });
});
