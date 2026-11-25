import { Suspense, lazy, useEffect, type ComponentType, type ReactNode } from 'react';
import { BrowserRouter, Navigate, Route, Routes, useLocation } from 'react-router';
import { MutationCache, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AuthProvider } from '@/context/AuthProvider';
import { useAuth } from '@/context/AuthContext';
import { ThemeProvider } from '@/context/ThemeProvider';
import { PwaProvider } from '@/context/PwaProvider';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { ToastProvider } from '@/components/ui/Toast';
import { ConfirmProvider } from '@/components/ui/ConfirmDialog';
import { PromptProvider } from '@/components/ui/PromptDialog';
import { OfflineBanner } from '@/components/ui/OfflineBanner';
import { toast } from '@/lib/toast';
import { ForbiddenPage, NotFoundPage } from '@/pages/errors/ErrorPage';
import { SUPER_ADMIN_ROLES, hasAnyRole } from '@/lib/roles';
import { ROUTES } from '@/lib/routes';
import { getErrorMessage } from '@/lib/utils';
import { userCanAny } from '@/lib/usePermissions';
import type { PermissionKey } from '@/api/permissions';

function lazyNamed<T extends ComponentType<Record<string, never>>>(
  loader: () => Promise<Record<string, T>>,
  exportName: string,
) {
  return lazy(async () => ({ default: (await loader())[exportName] }));
}

const AppLayout = lazyNamed(() => import('@/components/layout/AppLayout'), 'AppLayout');
const PortalLayout = lazyNamed(() => import('@/components/layout/PortalLayout'), 'PortalLayout');
const CommandPalette = lazyNamed(() => import('@/components/ui/CommandPalette'), 'CommandPalette');
const Login = lazyNamed(() => import('@/pages/auth/Login'), 'Login');
const OAuth2Callback = lazyNamed(() => import('@/pages/auth/OAuth2Callback'), 'OAuth2Callback');
const OAuth2Link = lazyNamed(() => import('@/pages/auth/OAuth2Link'), 'OAuth2Link');
const ForgotPassword = lazyNamed(() => import('@/pages/auth/ForgotPassword'), 'ForgotPassword');
const ResetPassword = lazyNamed(() => import('@/pages/auth/ResetPassword'), 'ResetPassword');
const ChangePassword = lazyNamed(() => import('@/pages/auth/ChangePassword'), 'ChangePassword');
const DashboardPage = lazyNamed(() => import('@/pages/dashboard/DashboardPage'), 'DashboardPage');
const EmployeeList = lazyNamed(() => import('@/pages/employees/EmployeeList'), 'EmployeeList');
const EmployeeCreate = lazyNamed(() => import('@/pages/employees/EmployeeCreate'), 'EmployeeCreate');
const EmployeeDetail = lazyNamed(() => import('@/pages/employees/EmployeeDetail'), 'EmployeeDetail');
const EmployeeImport = lazyNamed(() => import('@/pages/employees/EmployeeImport'), 'EmployeeImport');
const PayrollList = lazyNamed(() => import('@/pages/payroll/PayrollList'), 'PayrollList');
const PayrollOverview = lazyNamed(() => import('@/pages/payroll/PayrollOverview'), 'PayrollOverview');
const PayrollProcess = lazyNamed(() => import('@/pages/payroll/PayrollProcess'), 'PayrollProcess');
const PayrollDetail = lazyNamed(() => import('@/pages/payroll/PayrollDetail'), 'PayrollDetail');
const DocumentList = lazyNamed(() => import('@/pages/documents/DocumentList'), 'DocumentList');
const CompanyProfile = lazyNamed(() => import('@/pages/company/CompanyProfile'), 'CompanyProfile');
const SettingsPage = lazyNamed(() => import('@/pages/settings/SettingsPage'), 'SettingsPage');
const AccountSecurityPage = lazyNamed(
  () => import('@/pages/settings/AccountSecurityPage'),
  'AccountSecurityPage',
);
const Approvals = lazyNamed(() => import('@/pages/approvals/Approvals'), 'Approvals');
const Reports = lazyNamed(() => import('@/pages/reports/Reports'), 'Reports');
const CalendarPage = lazyNamed(() => import('@/pages/calendar/CalendarPage'), 'CalendarPage');
const TeamsPage = lazyNamed(() => import('@/pages/teams/TeamsPage'), 'TeamsPage');
const CompanyManagement = lazyNamed(() => import('@/pages/admin/CompanyManagement'), 'CompanyManagement');
const UserManagement = lazyNamed(() => import('@/pages/admin/UserManagement'), 'UserManagement');
const RoleManagement = lazyNamed(() => import('@/pages/admin/RoleManagement'), 'RoleManagement');
const UserGroups = lazyNamed(() => import('@/pages/admin/UserGroups'), 'UserGroups');
const AttendanceSettings = lazyNamed(() => import('@/pages/admin/AttendanceSettings'), 'AttendanceSettings');
const LettersPage = lazyNamed(() => import('@/pages/letters/LettersPage'), 'LettersPage');
const BackupPage = lazyNamed(() => import('@/pages/backup/BackupPage'), 'BackupPage');
const AuditTrailPage = lazyNamed(() => import('@/pages/audit/AuditTrailPage'), 'AuditTrailPage');
const AttendancePage = lazyNamed(() => import('@/pages/attendance/AttendancePage'), 'AttendancePage');
const AttendanceKioskPublic = lazyNamed(
  () => import('@/pages/attendance/AttendanceKioskPublic'),
  'AttendanceKioskPublic',
);
const AttendanceScanPage = lazyNamed(() => import('@/pages/attendance/AttendanceScanPage'), 'AttendanceScanPage');
const MyProfile = lazyNamed(() => import('@/pages/portal/MyProfile'), 'MyProfile');
const MyPayslips = lazyNamed(() => import('@/pages/portal/MyPayslips'), 'MyPayslips');
const Leave = lazyNamed(() => import('@/pages/portal/Leave'), 'Leave');
const Claims = lazyNamed(() => import('@/pages/portal/Claims'), 'Claims');
const Overtime = lazyNamed(() => import('@/pages/portal/Overtime'), 'Overtime');
const TeamCalendar = lazyNamed(() => import('@/pages/portal/TeamCalendar'), 'TeamCalendar');
const Notifications = lazyNamed(() => import('@/pages/portal/Notifications'), 'Notifications');
const MyAttendance = lazyNamed(() => import('@/pages/portal/MyAttendance'), 'MyAttendance');
const LegalPage = lazyNamed(() => import('@/pages/legal/LegalPage'), 'LegalPage');

function RouteFallback() {
  return (
    <div className="flex min-h-40 items-center justify-center">
      <div className="spinner" />
    </div>
  );
}

/**
 * Route guard keyed on capabilities rather than role names.
 *
 * `requires` is satisfied by holding *any* of the listed permissions. The
 * previous role-list version needed a special case to stop `exec` slipping into
 * payroll routes through a second role, and its `REPORT_ROLES` list included
 * `employee` — so a self-service account reached `/reports` and only then hit a
 * 403 from the API. Neither problem survives the move to permissions: `exec` and
 * `employee` simply hold no payroll or report permission.
 *
 * Presentation only. The API re-checks every permission on the request itself.
 */
export function PermissionGuard({
  requires,
  children,
}: {
  requires: PermissionKey | PermissionKey[];
  children: ReactNode;
}) {
  const { user } = useAuth();
  const location = useLocation();
  const needed = Array.isArray(requires) ? requires : [requires];
  if (user && !userCanAny(user, needed)) {
    return <Navigate to={ROUTES.FORBIDDEN} replace state={{ from: location.pathname }} />;
  }
  return <>{children}</>;
}

function HomeRedirect() {
  const { user } = useAuth();
  if (hasAnyRole(user, SUPER_ADMIN_ROLES)) {
    return <Navigate to={ROUTES.COMPANIES} replace />;
  }
  return <Navigate to={ROUTES.DASHBOARD} replace />;
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 30_000,
      refetchOnWindowFocus: false,
    },
  },
  // Safety net: surface unhandled mutation errors globally via ToastProvider
  // while still logging to console for diagnostics.
  mutationCache: new MutationCache({
    onError: (error, _variables, _context, mutation) => {
      if (!mutation.options.onError) {
        console.error('Unhandled mutation error', error);
        toast.error(getErrorMessage(error));
      }
    },
  }),
});

/**
 * Page-level boundary, keyed on the path.
 *
 * Remounting on navigation is the whole point: without the key a single crashed
 * route would leave the user staring at the fallback for the rest of the
 * session, because the boundary has no reason to reset itself.
 */
function RouteErrorBoundary({ children }: { children: ReactNode }) {
  const location = useLocation();
  return <ErrorBoundary key={location.pathname}>{children}</ErrorBoundary>;
}

function AuthenticatedCommandPalette() {
  const { isAuthenticated } = useAuth();
  if (!isAuthenticated) return null;
  return (
    <Suspense fallback={null}>
      <CommandPalette />
    </Suspense>
  );
}

let didPrefetchInitialRoute = false;
function prefetchInitialRoute() {
  if (didPrefetchInitialRoute || typeof window === 'undefined') return;
  didPrefetchInitialRoute = true;
  const path = window.location.pathname;
  if (path === '/login') {
    void import('@/pages/auth/Login');
    return;
  }
  if (path === '/terms' || path === '/privacy' || path === '/compliance') {
    void import('@/pages/legal/LegalPage');
    return;
  }
  if (path === '/kiosk' || path.startsWith('/kiosk/')) {
    void import('@/pages/attendance/AttendanceKioskPublic');
    return;
  }
  if (path.startsWith('/portal')) {
    void import('@/components/layout/PortalLayout');
    if (path === '/portal/attendance') void import('@/pages/portal/MyAttendance');
    else if (path === '/portal/payslips') void import('@/pages/portal/MyPayslips');
    else if (path === '/portal/leave') void import('@/pages/portal/Leave');
    else void import('@/pages/portal/MyProfile');
    return;
  }
  try {
    const rawUser = localStorage.getItem('user');
    if (!rawUser) {
      if (path === '/' || path === '') {
        void import('@/pages/auth/Login');
      }
      return;
    }
    const parsed = JSON.parse(rawUser) as { roles?: string[] };
    const roles = parsed.roles ?? [];
    if (roles.length === 1 && roles[0] === 'employee') {
      void import('@/components/layout/PortalLayout');
      void import('@/pages/portal/MyProfile');
    } else {
      void import('@/components/layout/AppLayout');
      if (roles.includes('super_admin')) {
        void import('@/pages/admin/CompanyManagement');
      } else if (path === '/' || path === '/dashboard') {
        void import('@/pages/dashboard/DashboardPage');
      }
    }
  } catch {
    // Ignore malformed localStorage display user cache.
  }
}

export default function App() {
  useEffect(() => {
    prefetchInitialRoute();
  }, []);

  return (
    <ThemeProvider>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <AuthProvider>
            <ToastProvider>
              <PwaProvider>
                <ConfirmProvider>
                  <PromptProvider>
                    <OfflineBanner />
                    <AuthenticatedCommandPalette />
                    <RouteErrorBoundary>
                    <Suspense fallback={<RouteFallback />}>
                    <Routes>
                      <Route path={ROUTES.LOGIN} element={<Login />} />
                      <Route path={ROUTES.OAUTH2_CALLBACK} element={<OAuth2Callback />} />
                      <Route path={ROUTES.OAUTH2_LINK} element={<OAuth2Link />} />
                      <Route path={ROUTES.FORGOT_PASSWORD} element={<ForgotPassword />} />
                      <Route path={ROUTES.RESET_PASSWORD} element={<ResetPassword />} />
                      <Route path={ROUTES.CHANGE_PASSWORD} element={<ChangePassword />} />
                      {/* The kiosk display is /kiosk#<secret> (with /kiosk/:kioskKey kept for legacy links). */}
                      <Route path={ROUTES.ATTENDANCE_SCAN} element={<AttendanceScanPage />} />
                      <Route path={ROUTES.KIOSK} element={<AttendanceKioskPublic />} />
                      <Route path={ROUTES.KIOSK_LEGACY} element={<AttendanceKioskPublic />} />
                      <Route path={ROUTES.TERMS} element={<LegalPage />} />
                      <Route path={ROUTES.PRIVACY} element={<LegalPage />} />
                      <Route path={ROUTES.COMPLIANCE} element={<LegalPage />} />
                      <Route path={ROUTES.FORBIDDEN} element={<ForbiddenPage />} />

                      <Route element={<AppLayout />}>
                        <Route path={ROUTES.HOME} element={<HomeRedirect />} />
                        <Route path={ROUTES.DASHBOARD} element={<DashboardPage />} />
                        <Route path={ROUTES.COMPANY} element={<CompanyProfile />} />
                  <Route
                    path={ROUTES.EMPLOYEES}
                    element={(
                      <PermissionGuard requires="view_employees">
                        <EmployeeList />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.EMPLOYEES_NEW}
                    element={(
                      <PermissionGuard requires="manage_employees">
                        <EmployeeCreate />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.EMPLOYEES_IMPORT}
                    element={(
                      <PermissionGuard requires="import_employees">
                        <EmployeeImport />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.EMPLOYEE_DETAIL}
                    element={(
                      <PermissionGuard requires="view_employees">
                        <EmployeeDetail />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.PAYROLL}
                    element={(
                      <PermissionGuard requires="view_payroll">
                        <PayrollOverview />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.PAYROLL_RUNS}
                    element={(
                      <PermissionGuard requires="view_payroll">
                        <PayrollList />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.PAYROLL_PROCESS}
                    element={(
                      <PermissionGuard requires="manage_payroll_draft">
                        <PayrollProcess />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.PAYROLL_DETAIL}
                    element={(
                      <PermissionGuard requires="view_payroll">
                        <PayrollDetail />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.DOCUMENTS}
                    element={(
                      <PermissionGuard requires="view_documents">
                        <DocumentList />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.CALENDAR}
                    element={(
                      <PermissionGuard requires="view_calendar">
                        <CalendarPage />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.TEAMS}
                    element={(
                      <PermissionGuard requires="view_teams">
                        <TeamsPage />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.APPROVALS}
                    element={(
                      <PermissionGuard requires="view_approvals">
                        <Approvals />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.REPORTS}
                    element={(
                      <PermissionGuard requires="view_reports">
                        <Reports />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.LETTERS}
                    element={(
                      <PermissionGuard requires="view_email_logs">
                        <LettersPage />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.SETTINGS}
                    element={(
                      <PermissionGuard requires="manage_company_settings">
                        <SettingsPage />
                      </PermissionGuard>
                    )}
                  />
                  <Route path={ROUTES.ACCOUNT} element={<AccountSecurityPage />} />
                  <Route
                    path={ROUTES.COMPANIES}
                    element={(
                      <PermissionGuard requires="manage_companies">
                        <CompanyManagement />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.USERS}
                    element={(
                      <PermissionGuard requires="manage_users">
                        <UserManagement />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.ROLES}
                    element={(
                      <PermissionGuard requires="view_user_directory">
                        <RoleManagement />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.USER_GROUPS}
                    element={(
                      <PermissionGuard requires="manage_users">
                        <UserGroups />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.BACKUP}
                    element={(
                      <PermissionGuard requires="manage_backups">
                        <BackupPage />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.AUDIT_TRAIL}
                    element={(
                      <PermissionGuard requires="view_audit_log">
                        <AuditTrailPage />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.ATTENDANCE}
                    element={(
                      <PermissionGuard requires="view_attendance">
                        <AttendancePage />
                      </PermissionGuard>
                    )}
                  />
                  <Route
                    path={ROUTES.ADMIN_ATTENDANCE_SETTINGS}
                    element={(
                      <PermissionGuard requires="manage_platform_settings">
                        <AttendanceSettings />
                      </PermissionGuard>
                    )}
                  />
                  <Route path={ROUTES.MY_ATTENDANCE} element={<MyAttendance />} />
                </Route>

                <Route element={<PortalLayout />}>
                  <Route path={ROUTES.PORTAL} element={<Navigate to={ROUTES.PORTAL_PROFILE} replace />} />
                  <Route path={ROUTES.PORTAL_PROFILE} element={<MyProfile />} />
                  <Route path={ROUTES.PORTAL_PAYSLIPS} element={<MyPayslips />} />
                  <Route path={ROUTES.PORTAL_LEAVE} element={<Leave />} />
                  <Route path={ROUTES.PORTAL_CLAIMS} element={<Claims />} />
                  <Route path={ROUTES.PORTAL_OVERTIME} element={<Overtime />} />
                  <Route path={ROUTES.PORTAL_TEAM_CALENDAR} element={<TeamCalendar />} />
                  <Route path={ROUTES.PORTAL_NOTIFICATIONS} element={<Notifications />} />
                  <Route path={ROUTES.PORTAL_ATTENDANCE} element={<MyAttendance />} />
                </Route>

                      <Route path="*" element={<NotFoundPage />} />
                    </Routes>
                  </Suspense>
                </RouteErrorBoundary>
                  </PromptProvider>
                </ConfirmProvider>
              </PwaProvider>
            </ToastProvider>
          </AuthProvider>
        </BrowserRouter>
      </QueryClientProvider>
    </ThemeProvider>
  );
}
