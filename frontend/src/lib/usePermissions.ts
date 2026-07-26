import { useAuth } from '@/context/AuthContext';
import type { PermissionKey } from '@/api/permissions';

/**
 * The caller's effective permissions.
 *
 * Read straight off the session `user`, which the backend populates from
 * `Permission::ALL` — so this is synchronous and cannot disagree with the API
 * about who may do what. UI gating reads from here rather than from role names:
 * a role's grants can then change in `backend/src/core/permission.rs` alone,
 * where previously the same rules were restated in `lib/roles.ts` and again in
 * the Role Management table, with nothing keeping the three in agreement.
 *
 * Never a security boundary. Every permission is re-checked server-side on the
 * request that acts on it; this only decides what is worth rendering.
 */
export function usePermissions() {
  const { user } = useAuth();
  const granted = user?.permissions;

  return {
    permissions: granted,
    can: (permission: PermissionKey) => granted?.includes(permission) ?? false,
    canAny: (permissions: PermissionKey[]) =>
      permissions.some((permission) => granted?.includes(permission) ?? false),
    canAll: (permissions: PermissionKey[]) =>
      permissions.every((permission) => granted?.includes(permission) ?? false),
  };
}

/**
 * Permission check for code that already holds the user (route guards, tests)
 * and cannot call a hook.
 */
export function userCan(
  user: { permissions?: PermissionKey[] } | null | undefined,
  permission: PermissionKey,
): boolean {
  return user?.permissions?.includes(permission) ?? false;
}

export function userCanAny(
  user: { permissions?: PermissionKey[] } | null | undefined,
  permissions: PermissionKey[],
): boolean {
  return permissions.some((permission) => userCan(user, permission));
}
