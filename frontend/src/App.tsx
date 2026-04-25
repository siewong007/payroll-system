import { Suspense, lazy, type ComponentType, type ReactNode } from 'react';
import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AuthProvider } from '@/context/AuthProvider';
import { useAuth } from '@/context/AuthContext';
import { AppLayout } from '@/components/layout/AppLayout';
import { PortalLayout } from '@/components/layout/PortalLayout';
import {
  ADMIN_DATA_ROLES,
  PAYROLL_DATA_ROLES,
  PAYROLL_PREP_ROLES,
  REPORT_ROLES,
  SUPER_ADMIN_ROLES,
  type AppRole,
} from '@/lib/roles';

function lazyNamed<T extends ComponentType<Record<string, never>>>(
  loader: () => Promise<Record<string, T>>,
  exportName: string,
) {
  return lazy(async () => ({ default: (await loader())[exportName] }));
}

const Login = lazyNamed(() => import('@/pages/auth/Login'), 'Login');
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
const AttendanceKiosk = lazyNamed(() => import('@/pages/attendance/AttendanceKiosk'), 'AttendanceKiosk');
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
      <div className="h-7 w-7 animate-spin rounded-full border-b-2 border-gray-900" />
    </div>
  );
}

function RoleGuard({ allowedRoles, children }: { allowedRoles: AppRole[]; children: ReactNode }) {
  const { user } = useAuth();
  if (user && !allowedRoles.includes(user.role)) {
    return <Navigate to={user.role === 'super_admin' ? '/companies' : '/company'} replace />;
  }
  return <>{children}</>;
}

function HomeRedirect() {
  const { user } = useAuth();
  if (user?.role === 'super_admin') {
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
              <Route path="/forgot-password" element={<ForgotPassword />} />
              <Route path="/reset-password" element={<ResetPassword />} />
              <Route path="/change-password" element={<ChangePassword />} />
              <Route path="/attendance/kiosk" element={<AttendanceKiosk />} />
              <Route path="/attendance/scan" element={<AttendanceScanPage />} />
              <Route path="/kiosk/:kioskKey" element={<AttendanceKioskPublic />} />

              <Route element={<AppLayout />}>
                <Route path="/" element={<HomeRedirect />} />
                <Route path="/company" element={<CompanyProfile />} />
                <Route path="/employees" element={<EmployeeList />} />
                <Route path="/employees/new" element={<EmployeeCreate />} />
                <Route
                  path="/employees/import"
                  element={(
                    <RoleGuard allowedRoles={PAYROLL_PREP_ROLES}>
                      <EmployeeImport />
                    </RoleGuard>
                  )}
                />
                <Route path="/employees/:id" element={<EmployeeDetail />} />
                <Route
                  path="/payroll"
                  element={(
                    <RoleGuard allowedRoles={PAYROLL_DATA_ROLES}>
                      <PayrollList />
                    </RoleGuard>
                  )}
                />
                <Route
                  path="/payroll/process"
                  element={(
                    <RoleGuard allowedRoles={PAYROLL_PREP_ROLES}>
                      <PayrollProcess />
                    </RoleGuard>
                  )}
                />
                <Route
                  path="/payroll/:id"
                  element={(
                    <RoleGuard allowedRoles={PAYROLL_DATA_ROLES}>
                      <PayrollDetail />
                    </RoleGuard>
                  )}
                />
                <Route path="/documents" element={<DocumentList />} />
                <Route path="/calendar" element={<CalendarPage />} />
                <Route path="/teams" element={<TeamsPage />} />
                <Route path="/approvals" element={<Approvals />} />
                <Route
                  path="/reports"
                  element={(
                    <RoleGuard allowedRoles={REPORT_ROLES}>
                      <Reports />
                    </RoleGuard>
                  )}
                />
                <Route path="/letters" element={<LettersPage />} />
                <Route path="/settings" element={<SettingsPage />} />
                <Route
                  path="/companies"
                  element={(
                    <RoleGuard allowedRoles={SUPER_ADMIN_ROLES}>
                      <CompanyManagement />
                    </RoleGuard>
                  )}
                />
                <Route
                  path="/users"
                  element={(
                    <RoleGuard allowedRoles={SUPER_ADMIN_ROLES}>
                      <UserManagement />
                    </RoleGuard>
                  )}
                />
                <Route
                  path="/roles"
                  element={(
                    <RoleGuard allowedRoles={SUPER_ADMIN_ROLES}>
                      <RoleManagement />
                    </RoleGuard>
                  )}
                />
                <Route
                  path="/backup"
                  element={(
                    <RoleGuard allowedRoles={ADMIN_DATA_ROLES}>
                      <BackupPage />
                    </RoleGuard>
                  )}
                />
                <Route
                  path="/audit-trail"
                  element={(
                    <RoleGuard allowedRoles={ADMIN_DATA_ROLES}>
                      <AuditTrailPage />
                    </RoleGuard>
                  )}
                />
                <Route path="/attendance" element={<AttendancePage />} />
                <Route
                  path="/admin/attendance-settings"
                  element={(
                    <RoleGuard allowedRoles={SUPER_ADMIN_ROLES}>
                      <AttendanceSettings />
                    </RoleGuard>
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
            </Routes>
          </Suspense>
        </AuthProvider>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
