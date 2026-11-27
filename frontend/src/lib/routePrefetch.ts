const prefetched = new Set<string>();

function loadOnce(key: string, loader: () => Promise<unknown>) {
  if (prefetched.has(key)) return;
  prefetched.add(key);
  void loader();
}

const ROUTE_LOADERS: ReadonlyArray<{ prefix: string; exact?: boolean; load: () => Promise<unknown> }> = [
  { prefix: '/login', exact: true, load: () => import('@/pages/auth/Login') },
  { prefix: '/forgot-password', exact: true, load: () => import('@/pages/auth/ForgotPassword') },
  { prefix: '/reset-password', exact: true, load: () => import('@/pages/auth/ResetPassword') },
  { prefix: '/change-password', exact: true, load: () => import('@/pages/auth/ChangePassword') },
  { prefix: '/terms', exact: true, load: () => import('@/pages/legal/LegalPage') },
  { prefix: '/privacy', exact: true, load: () => import('@/pages/legal/LegalPage') },
  { prefix: '/compliance', exact: true, load: () => import('@/pages/legal/LegalPage') },
  { prefix: '/kiosk', load: () => import('@/pages/attendance/AttendanceKioskPublic') },
  { prefix: '/attendance/scan', load: () => import('@/pages/attendance/AttendanceScanPage') },
  { prefix: '/dashboard', load: () => import('@/pages/dashboard/DashboardPage') },
  { prefix: '/company', exact: true, load: () => import('@/pages/company/CompanyProfile') },
  { prefix: '/employees/new', exact: true, load: () => import('@/pages/employees/EmployeeCreate') },
  { prefix: '/employees/import', exact: true, load: () => import('@/pages/employees/EmployeeImport') },
  { prefix: '/employees', exact: true, load: () => import('@/pages/employees/EmployeeList') },
  { prefix: '/employees/', load: () => import('@/pages/employees/EmployeeDetail') },
  { prefix: '/payroll/runs', exact: true, load: () => import('@/pages/payroll/PayrollList') },
  { prefix: '/payroll/process', exact: true, load: () => import('@/pages/payroll/PayrollProcess') },
  { prefix: '/payroll/', load: () => import('@/pages/payroll/PayrollDetail') },
  { prefix: '/payroll', exact: true, load: () => import('@/pages/payroll/PayrollOverview') },
  { prefix: '/teams', load: () => import('@/pages/teams/TeamsPage') },
  { prefix: '/calendar', load: () => import('@/pages/calendar/CalendarPage') },
  { prefix: '/attendance', load: () => import('@/pages/attendance/AttendancePage') },
  { prefix: '/approvals', load: () => import('@/pages/approvals/Approvals') },
  { prefix: '/reports', load: () => import('@/pages/reports/Reports') },
  { prefix: '/documents', load: () => import('@/pages/documents/DocumentList') },
  { prefix: '/letters', load: () => import('@/pages/letters/LettersPage') },
  { prefix: '/settings', load: () => import('@/pages/settings/SettingsPage') },
  { prefix: '/account', load: () => import('@/pages/settings/AccountSecurityPage') },
  { prefix: '/companies', load: () => import('@/pages/admin/CompanyManagement') },
  { prefix: '/users', load: () => import('@/pages/admin/UserManagement') },
  { prefix: '/roles', load: () => import('@/pages/admin/RoleManagement') },
  { prefix: '/user-groups', load: () => import('@/pages/admin/UserGroups') },
  { prefix: '/admin/attendance-settings', load: () => import('@/pages/admin/AttendanceSettings') },
  { prefix: '/admin/platform', load: () => import('@/pages/admin/PlatformSettingsPage') },
  { prefix: '/audit-trail', load: () => import('@/pages/audit/AuditTrailPage') },
  { prefix: '/backup', load: () => import('@/pages/backup/BackupPage') },
  { prefix: '/my/attendance', load: () => import('@/pages/portal/MyAttendance') },
  { prefix: '/notifications', load: () => import('@/pages/portal/Notifications') },
  { prefix: '/portal/profile', load: () => import('@/pages/portal/MyProfile') },
  { prefix: '/portal/payslips', load: () => import('@/pages/portal/MyPayslips') },
  { prefix: '/portal/leave', load: () => import('@/pages/portal/Leave') },
  { prefix: '/portal/attendance', load: () => import('@/pages/portal/MyAttendance') },
  { prefix: '/portal/claims', load: () => import('@/pages/portal/Claims') },
  { prefix: '/portal/overtime', load: () => import('@/pages/portal/Overtime') },
  { prefix: '/portal/team-calendar', load: () => import('@/pages/portal/TeamCalendar') },
  { prefix: '/portal/notifications', load: () => import('@/pages/portal/Notifications') },
];

export function prefetchRoute(href: string) {
  const path = href.split('?')[0].split('#')[0];
  for (const entry of ROUTE_LOADERS) {
    if (entry.exact ? path === entry.prefix : path.startsWith(entry.prefix)) {
      loadOnce(entry.prefix, entry.load);
      return;
    }
  }
}

let didPrefetchInitial = false;

export function prefetchInitialRoute() {
  if (didPrefetchInitial || typeof window === 'undefined') return;
  didPrefetchInitial = true;

  const path = window.location.pathname;
  if (
    path === '/login' ||
    path === '/terms' ||
    path === '/privacy' ||
    path === '/compliance' ||
    path === '/kiosk' ||
    path.startsWith('/kiosk/') ||
    path.startsWith('/attendance/scan')
  ) {
    prefetchRoute(path);
    return;
  }

  if (path.startsWith('/portal')) {
    loadOnce('layout:portal', () => import('@/components/layout/PortalLayout'));
    prefetchRoute(path === '/portal' ? '/portal/profile' : path);
    return;
  }

  try {
    const rawUser = localStorage.getItem('user');
    if (!rawUser) {
      if (path === '/' || path === '') {
        prefetchRoute('/login');
      } else {
        prefetchRoute(path);
      }
      return;
    }
    const parsed = JSON.parse(rawUser) as { roles?: string[] };
    const roles = parsed.roles ?? [];
    if (roles.length === 1 && roles[0] === 'employee') {
      loadOnce('layout:portal', () => import('@/components/layout/PortalLayout'));
      prefetchRoute(path === '/portal/notifications' ? path : '/portal/profile');
      return;
    }

    loadOnce('layout:admin', () => import('@/components/layout/AppLayout'));
    if (path === '/' || path === '/dashboard') {
      if (roles.includes('super_admin')) {
        prefetchRoute('/companies');
      } else {
        prefetchRoute('/dashboard');
      }
    } else {
      prefetchRoute(path);
    }
  } catch {
    // Ignore malformed localStorage display user cache.
  }
}
