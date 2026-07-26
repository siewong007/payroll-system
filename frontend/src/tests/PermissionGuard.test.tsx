import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PermissionKey } from '@/api/permissions';
import type { AppRole, User } from '@/types';
import { userWithRoles } from './support/permissions';

const authMocks = vi.hoisted(() => ({ useAuth: vi.fn() }));

vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));

// App.tsx pulls in every page through React.lazy. lazy() only registers a
// loader — the dynamic import is not executed until the component renders —
// so importing App here stays cheap and none of the pages are evaluated.
import { PermissionGuard } from '@/App';

function renderGuard(requires: PermissionKey | PermissionKey[], user: User | null) {
  authMocks.useAuth.mockReturnValue({ user });
  return render(
    <MemoryRouter initialEntries={['/protected']}>
      <Routes>
        <Route
          path="/protected"
          element={(
            <PermissionGuard requires={requires}>
              <div>Protected content</div>
            </PermissionGuard>
          )}
        />
        <Route path="/403" element={<div>Forbidden</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

/** Renders in isolation and tears down, so a matrix case can call it in a loop
 *  without earlier renders piling up in the same document. */
function isAllowed(requires: PermissionKey | PermissionKey[], roles: AppRole[]): boolean {
  const { unmount } = renderGuard(requires, userWithRoles(roles));
  const allowed = screen.queryByText('Protected content') !== null;
  unmount();
  return allowed;
}

beforeEach(() => {
  authMocks.useAuth.mockReset();
});

describe('PermissionGuard', () => {
  it('renders children when the permission is held', () => {
    expect(isAllowed('view_payroll', ['payroll_admin'])).toBe(true);
  });

  it('redirects to /403 when the permission is missing', () => {
    renderGuard('view_payroll', userWithRoles(['hr_manager']));

    expect(screen.queryByText('Protected content')).not.toBeInTheDocument();
    expect(screen.getByText('Forbidden')).toBeInTheDocument();
  });

  it('renders children while the session is still resolving', () => {
    // `user` is null during the initial /auth/refresh. Redirecting here would
    // bounce every authorized user to /403 on a hard refresh, so the guard
    // waits and lets the layout/route handle the unauthenticated case.
    renderGuard('manage_users', null);

    expect(screen.getByText('Protected content')).toBeInTheDocument();
  });

  it('treats a missing permissions array as holding nothing', () => {
    // A session restored from a localStorage mirror written by an older build
    // has no `permissions`. It must fail closed rather than render.
    const stale: User = { ...userWithRoles(['super_admin']), permissions: undefined };
    renderGuard('manage_users', stale);

    expect(screen.queryByText('Protected content')).not.toBeInTheDocument();
  });

  it('accepts any one of several listed permissions', () => {
    expect(isAllowed(['manage_backups', 'view_audit_log'], ['admin'])).toBe(true);
  });

  it('grants access on the union of several held roles', () => {
    // Additive: payroll_admin carries view_payroll even though hr_manager alone
    // would not.
    expect(isAllowed('view_payroll', ['hr_manager', 'payroll_admin'])).toBe(true);
  });
});

describe('PermissionGuard route matrix', () => {
  // One row per guarded route in App.tsx: the permission the route requires,
  // and the roles that consequently reach it. The role-to-permission grants
  // themselves are asserted in the backend (`core::permission` unit tests and
  // `route_auth_tests`); this pins the *route to permission* mapping, which is
  // the part that lives in App.tsx.
  const matrix: {
    route: string;
    requires: PermissionKey;
    admits: AppRole[];
    denies: AppRole[];
  }[] = [
    {
      route: '/payroll, /payroll/:id',
      requires: 'view_payroll',
      admits: ['super_admin', 'payroll_admin', 'finance'],
      denies: ['admin', 'hr_manager', 'exec', 'employee'],
    },
    {
      route: '/payroll/process',
      requires: 'manage_payroll_draft',
      admits: ['super_admin', 'payroll_admin'],
      // finance approves but must not prepare — separation of duties.
      denies: ['finance', 'admin', 'hr_manager', 'exec', 'employee'],
    },
    {
      route: '/employees/import',
      requires: 'import_employees',
      admits: ['super_admin', 'payroll_admin'],
      denies: ['admin', 'hr_manager', 'finance', 'exec', 'employee'],
    },
    {
      route: '/reports',
      requires: 'view_reports',
      admits: ['super_admin', 'admin', 'payroll_admin', 'hr_manager', 'finance'],
      // exec is read-mostly but never sees reports; `employee` used to reach
      // this route because it appeared in REPORT_ROLES.
      denies: ['exec', 'employee'],
    },
    {
      route: '/companies',
      requires: 'manage_companies',
      admits: ['super_admin'],
      denies: ['admin', 'payroll_admin', 'hr_manager', 'finance', 'exec', 'employee'],
    },
    {
      route: '/users, /roles',
      requires: 'manage_users',
      admits: ['super_admin'],
      denies: ['admin', 'payroll_admin', 'hr_manager', 'finance', 'exec', 'employee'],
    },
    {
      route: '/backup',
      requires: 'manage_backups',
      // The archive carries payroll_items, salary_history and raw employee
      // rows, so it is super_admin alone — `admin` was previously offered the
      // page and got a 403 from the API.
      admits: ['super_admin'],
      denies: ['admin', 'payroll_admin', 'hr_manager', 'finance', 'exec', 'employee'],
    },
    {
      route: '/audit-trail',
      requires: 'view_audit_log',
      admits: ['super_admin', 'admin'],
      denies: ['payroll_admin', 'hr_manager', 'finance', 'exec', 'employee'],
    },
    {
      route: '/attendance',
      requires: 'view_attendance',
      admits: ['super_admin', 'admin', 'hr_manager', 'payroll_admin', 'finance', 'exec'],
      // Self-service employees read their own attendance at /portal/attendance,
      // never the company-wide view with colleagues' GPS coordinates.
      denies: ['employee'],
    },
    {
      route: '/teams',
      requires: 'view_teams',
      admits: ['super_admin', 'admin', 'payroll_admin', 'hr_manager', 'exec'],
      denies: ['finance', 'employee'],
    },
    {
      route: '/settings',
      requires: 'manage_company_settings',
      admits: ['super_admin', 'admin'],
      denies: ['payroll_admin', 'hr_manager', 'finance', 'exec', 'employee'],
    },
    {
      route: '/admin/attendance-settings',
      requires: 'manage_platform_settings',
      admits: ['super_admin'],
      denies: ['admin', 'payroll_admin', 'hr_manager', 'finance', 'exec', 'employee'],
    },
  ];

  it.each(matrix)('$route admits exactly the roles holding $requires', ({ requires, admits, denies }) => {
    for (const role of admits) {
      expect({ role, allowed: isAllowed(requires, [role]) }).toEqual({ role, allowed: true });
    }
    for (const role of denies) {
      expect({ role, allowed: isAllowed(requires, [role]) }).toEqual({ role, allowed: false });
    }
  });

  it('never admits a self-service employee to an admin route', () => {
    for (const { requires } of matrix) {
      expect({ requires, allowed: isAllowed(requires, ['employee']) }).toEqual({
        requires,
        allowed: false,
      });
    }
  });
});
