import { Suspense, lazy, type ComponentType, type ReactNode } from 'react';
import { BrowserRouter, Navigate, Route, Routes, useLocation } from 'react-router';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AuthProvider } from '@/context/AuthProvider';
import { useAuth } from '@/context/AuthContext';
import { AppLayout } from '@/components/layout/AppLayout';
import { PortalLayout } from '@/components/layout/PortalLayout';
import { ForbiddenPage, NotFoundPage } from '@/pages/errors/ErrorPage';
import { SUPER_ADMIN_ROLES, hasAnyRole } from '@/lib/roles';
import { userCanAny } from '@/lib/usePermissions';
import type { PermissionKey } from '@/api/permissions';

function lazyNamed<T extends ComponentType<Record<string, never>>>(
  loader: () => Promise<Record<string, T>>,
  exportName: string,
) {
  return lazy(async () => ({ default: (await loader())[exportName] }));
}

const Login = lazyNamed(() => import('@/pages/auth/Login'), 'Login');
const OAuth2Callback = lazyNamed(() => import('@/pages/auth/OAuth2Callback'), 'OAuth2Callback');
const ForgotPassword = lazyNamed(() => import('@/pages/auth/ForgotPassword'), 'ForgotPassword');
const ResetPassword = lazyNamed(() => import('@/pages/auth/ResetPassword'), 'ResetPassword');
const ChangePassword = lazyNamed(() => import('@/pages/auth/ChangePassword'), 'ChangePassword');
const EmployeeList = lazyNamed(() => import('@/pages/employees/EmployeeList'), 'EmployeeList');
const EmployeeCreate = lazyNamed(() => import('@/pages/employees/EmployeeCreate'), 'EmployeeCreate');
const EmployeeDetail = lazyNamed(() => import('@/pages/employees/EmployeeDetail'), 'EmployeeDetail');
const EmployeeImport = lazyNamed(() => import('@/pages/employees/EmployeeImport'), 'EmployeeImport');
const PayrollList = lazyNamed(() => import('@/pages/payroll/PayrollList'), 'PayrollList');
const PayrollProcess = lazyNamed(() => import('@/pages/payroll/PayrollProcess'), 'PayrollProcess');
const PayrollDetail = lazyNamed(() => import('@/pages/payroll/PayrollDetail'), 'PayrollDetail');
const DocumentList = lazyNamed(() => import('@/pages/documents/DocumentList'), 'DocumentList');
const CompanyProfile = lazyNamed(() => import('@/pages/company/CompanyProfile'), 'CompanyProfile');
const SettingsPage = lazyNamed(() => import('@/pages/settings/SettingsPage'), 'SettingsPage');
const Approvals = lazyNamed(() => import('@/pages/approvals/Approvals'), 'Approvals');
const Reports = lazyNamed(() => import('@/pages/reports/Reports'), 'Reports');
const CalendarPage = lazyNamed(() => import('@/pages/calendar/CalendarPage'), 'CalendarPage');
const TeamsPage = lazyNamed(() => import('@/pages/teams/TeamsPage'), 'TeamsPage');
const CompanyManagement = lazyNamed(() => import('@/pages/admin/CompanyManagement'), 'CompanyManagement');
const UserManagement = lazyNamed(() => import('@/pages/admin/UserManagement'), 'UserManagement');
const RoleManagement = lazyNamed(() => import('@/pages/admin/RoleManagement'), 'RoleManagement');
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
    return <Navigate to="/403" replace state={{ from: location.pathname }} />;
  }
  return <>{children}</>;
}

function HomeRedirect() {
  const { user } = useAuth();
  if (hasAnyRole(user, SUPER_ADMIN_ROLES)) {
    return <Navigate to="/companies" replace />;
  }
  return <Navigate to="/company" replace />;
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 30_000,
      refetchOnWindowFocus: false,
    },
  },
});

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <AuthProvider>
          <Suspense fallback={<RouteFallback />}>
            <Routes>
              <Route path="/login" element={<Login />} />
              <Route path="/oauth2/callback" element={<OAuth2Callback />} />
              <Route path="/forgot-password" element={<ForgotPassword />} />
              <Route path="/reset-password" element={<ResetPassword />} />
              <Route path="/change-password" element={<ChangePassword />} />
              {/* The kiosk display is /kiosk/:kioskKey below — it needs no user
                  session, so a wall-mounted tablet never falls back to a login
                  screen. The old session-backed /attendance/kiosk is gone. */}
              <Route path="/attendance/scan" element={<AttendanceScanPage />} />
              <Route path="/kiosk/:kioskKey" element={<AttendanceKioskPublic />} />
              <Route path="/403" element={<ForbiddenPage />} />

              <Route element={<AppLayout />}>
                <Route path="/" element={<HomeRedirect />} />
                <Route path="/company" element={<CompanyProfile />} />
                <Route
                  path="/employees"
                  element={(
                    <PermissionGuard requires="view_employees">
                      <EmployeeList />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/employees/new"
                  element={(
                    <PermissionGuard requires="manage_employees">
                      <EmployeeCreate />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/employees/import"
                  element={(
                    <PermissionGuard requires="import_employees">
                      <EmployeeImport />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/employees/:id"
                  element={(
                    <PermissionGuard requires="view_employees">
                      <EmployeeDetail />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/payroll"
                  element={(
                    <PermissionGuard requires="view_payroll">
                      <PayrollList />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/payroll/process"
                  element={(
                    <PermissionGuard requires="manage_payroll_draft">
                      <PayrollProcess />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/payroll/:id"
                  element={(
                    <PermissionGuard requires="view_payroll">
                      <PayrollDetail />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/documents"
                  element={(
                    <PermissionGuard requires="view_documents">
                      <DocumentList />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/calendar"
                  element={(
                    <PermissionGuard requires="view_calendar">
                      <CalendarPage />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/teams"
                  element={(
                    <PermissionGuard requires="view_teams">
                      <TeamsPage />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/approvals"
                  element={(
                    <PermissionGuard requires="view_approvals">
                      <Approvals />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/reports"
                  element={(
                    <PermissionGuard requires="view_reports">
                      <Reports />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/letters"
                  element={(
                    <PermissionGuard requires="view_email_logs">
                      <LettersPage />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/settings"
                  element={(
                    <PermissionGuard requires="manage_company_settings">
                      <SettingsPage />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/companies"
                  element={(
                    <PermissionGuard requires="manage_companies">
                      <CompanyManagement />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/users"
                  element={(
                    <PermissionGuard requires="manage_users">
                      <UserManagement />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/roles"
                  element={(
                    <PermissionGuard requires="manage_users">
                      <RoleManagement />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/backup"
                  element={(
                    <PermissionGuard requires="manage_backups">
                      <BackupPage />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/audit-trail"
                  element={(
                    <PermissionGuard requires="view_audit_log">
                      <AuditTrailPage />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/attendance"
                  element={(
                    <PermissionGuard requires="view_attendance">
                      <AttendancePage />
                    </PermissionGuard>
                  )}
                />
                <Route
                  path="/admin/attendance-settings"
                  element={(
                    <PermissionGuard requires="manage_platform_settings">
                      <AttendanceSettings />
                    </PermissionGuard>
                  )}
                />
              </Route>

              <Route element={<PortalLayout />}>
                <Route path="/portal" element={<Navigate to="/portal/profile" replace />} />
                <Route path="/portal/profile" element={<MyProfile />} />
                <Route path="/portal/payslips" element={<MyPayslips />} />
                <Route path="/portal/leave" element={<Leave />} />
                <Route path="/portal/claims" element={<Claims />} />
                <Route path="/portal/overtime" element={<Overtime />} />
                <Route path="/portal/team-calendar" element={<TeamCalendar />} />
                <Route path="/portal/notifications" element={<Notifications />} />
                <Route path="/portal/attendance" element={<MyAttendance />} />
              </Route>

              <Route path="*" element={<NotFoundPage />} />
            </Routes>
          </Suspense>
        </AuthProvider>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
