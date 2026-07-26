import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppRole, User } from '@/types';
import {
  ADMIN_DATA_ROLES,
  ATTENDANCE_VIEW_ROLES,
  PAYROLL_DATA_ROLES,
  PAYROLL_PREP_ROLES,
  REPORT_ROLES,
  SUPER_ADMIN_ROLES,
} from '@/lib/roles';

const authMocks = vi.hoisted(() => ({ useAuth: vi.fn() }));

vi.mock('@/context/AuthContext', () => ({ useAuth: authMocks.useAuth }));

// App.tsx pulls in every page through React.lazy. lazy() only registers a
// loader — the dynamic import is not executed until the component renders —
// so importing App here stays cheap and none of the pages are evaluated.
import { RoleGuard } from '@/App';

function asUser(roles: AppRole[]): User {
  return {
    id: 'user-1',
    email: 'person@example.com',
    full_name: 'Test Person',
    roles,
    company_id: 'company-1',
    employee_id: null,
  };
}

function renderGuard(allowedRoles: AppRole[], user: User | null) {
  authMocks.useAuth.mockReturnValue({ user });
  return render(
    <MemoryRouter initialEntries={['/protected']}>
      <Routes>
        <Route
          path="/protected"
          element={(
            <RoleGuard allowedRoles={allowedRoles}>
              <div>Protected content</div>
            </RoleGuard>
          )}
        />
        <Route path="/403" element={<div>Forbidden</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

/** Renders in isolation and tears down, so a matrix case can call it in a loop
 *  without earlier renders piling up in the same document. */
function isAllowed(allowedRoles: AppRole[], roles: AppRole[]): boolean {
  const { unmount } = renderGuard(allowedRoles, asUser(roles));
  const allowed = screen.queryByText('Protected content') !== null;
  unmount();
  return allowed;
}

beforeEach(() => {
  authMocks.useAuth.mockReset();
});

describe('RoleGuard', () => {
  it('renders children for a permitted role', () => {
    expect(isAllowed(PAYROLL_DATA_ROLES, ['payroll_admin'])).toBe(true);
  });

  it('redirects a disallowed role to /403', () => {
    renderGuard(PAYROLL_DATA_ROLES, asUser(['hr_manager']));

    expect(screen.queryByText('Protected content')).not.toBeInTheDocument();
    expect(screen.getByText('Forbidden')).toBeInTheDocument();
  });

  it('renders children while the session is still resolving', () => {
    // `user` is null during the initial /auth/refresh. Redirecting here would
    // bounce every authorized user to /403 on a hard refresh, so the guard
    // waits and lets the layout/route handle the unauthenticated case.
    renderGuard(SUPER_ADMIN_ROLES, null);

    expect(screen.getByText('Protected content')).toBeInTheDocument();
  });

  it('grants access on any one of several held roles', () => {
    // Additive semantics: payroll_admin carries the route even though
    // hr_manager alone would not.
    expect(isAllowed(PAYROLL_DATA_ROLES, ['hr_manager', 'payroll_admin'])).toBe(true);
  });
});

describe('RoleGuard exec containment', () => {
  it('denies exec on a route that does not list it, even alongside a permitted role', () => {
    // The regression this encodes: hasAnyRole passes on a single match, so
    // ['exec','employee'] would otherwise reach /reports via `employee`.
    expect(isAllowed(REPORT_ROLES, ['exec', 'employee'])).toBe(false);
  });

  it('denies exec on payroll routes however it is combined', () => {
    expect(isAllowed(PAYROLL_DATA_ROLES, ['exec'])).toBe(false);
    expect(isAllowed(PAYROLL_DATA_ROLES, ['exec', 'finance'])).toBe(false);
    expect(isAllowed(PAYROLL_PREP_ROLES, ['exec', 'payroll_admin'])).toBe(false);
  });

  it('admits exec only where the route explicitly lists it', () => {
    // Attendance is the one exec-listed guarded route: read-mostly access.
    expect(ATTENDANCE_VIEW_ROLES).toContain('exec');
    expect(isAllowed(ATTENDANCE_VIEW_ROLES, ['exec'])).toBe(true);
  });

  it('does not penalize non-exec users on exec-less routes', () => {
    expect(isAllowed(REPORT_ROLES, ['employee'])).toBe(true);
  });
});

describe('RoleGuard route matrix', () => {
  // Each row mirrors a guarded route in App.tsx. A change to either side that
  // silently widens access breaks here.
  const matrix: { route: string; allowed: AppRole[]; admits: AppRole[]; denies: AppRole[] }[] = [
    {
      route: '/payroll',
      allowed: PAYROLL_DATA_ROLES,
      admits: ['super_admin', 'payroll_admin', 'finance'],
      denies: ['admin', 'hr_manager', 'exec', 'employee'],
    },
    {
      route: '/payroll/process',
      allowed: PAYROLL_PREP_ROLES,
      admits: ['super_admin', 'payroll_admin'],
      // finance approves but must not prepare — separation of duties.
      denies: ['finance', 'admin', 'hr_manager', 'exec', 'employee'],
    },
    {
      route: '/employees/import',
      allowed: PAYROLL_PREP_ROLES,
      admits: ['super_admin', 'payroll_admin'],
      denies: ['hr_manager', 'finance', 'exec', 'employee'],
    },
    {
      route: '/reports',
      allowed: REPORT_ROLES,
      admits: ['super_admin', 'admin', 'payroll_admin', 'hr_manager', 'finance', 'employee'],
      denies: ['exec'],
    },
    {
      route: '/companies, /users, /roles, /admin/attendance-settings',
      allowed: SUPER_ADMIN_ROLES,
      admits: ['super_admin'],
      denies: ['admin', 'payroll_admin', 'hr_manager', 'finance', 'exec', 'employee'],
    },
    {
      route: '/backup, /audit-trail',
      allowed: ADMIN_DATA_ROLES,
      admits: ['super_admin', 'admin'],
      denies: ['payroll_admin', 'hr_manager', 'finance', 'exec', 'employee'],
    },
    {
      route: '/attendance',
      allowed: ATTENDANCE_VIEW_ROLES,
      admits: ['super_admin', 'admin', 'hr_manager', 'payroll_admin', 'finance', 'exec'],
      // Self-service employees read their own attendance at /portal/attendance,
      // never the company-wide view with colleagues' GPS coordinates.
      denies: ['employee'],
    },
  ];

  it.each(matrix)('$route admits exactly its listed roles', ({ allowed, admits, denies }) => {
    for (const role of admits) {
      expect({ role, allowed: isAllowed(allowed, [role]) }).toEqual({ role, allowed: true });
    }
    for (const role of denies) {
      expect({ role, allowed: isAllowed(allowed, [role]) }).toEqual({ role, allowed: false });
    }
  });
});
