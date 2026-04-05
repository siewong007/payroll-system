import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AuthProvider, useAuth } from '@/context/AuthContext';
import { AppLayout } from '@/components/layout/AppLayout';
import { PortalLayout } from '@/components/layout/PortalLayout';
import { Login } from '@/pages/auth/Login';
import { ForgotPassword } from '@/pages/auth/ForgotPassword';
import { ResetPassword } from '@/pages/auth/ResetPassword';
import { Dashboard } from '@/pages/Dashboard';
import { EmployeeList } from '@/pages/employees/EmployeeList';
import { EmployeeCreate } from '@/pages/employees/EmployeeCreate';
import { EmployeeDetail } from '@/pages/employees/EmployeeDetail';
import { PayrollList } from '@/pages/payroll/PayrollList';
import { PayrollProcess } from '@/pages/payroll/PayrollProcess';
import { PayrollDetail } from '@/pages/payroll/PayrollDetail';
import { DocumentList } from '@/pages/documents/DocumentList';
import { CompanyProfile } from '@/pages/company/CompanyProfile';
import { SettingsPage } from '@/pages/settings/SettingsPage';
import { Approvals } from '@/pages/approvals/Approvals';
import { Reports } from '@/pages/reports/Reports';
import { PortalHome } from '@/pages/portal/PortalHome';
import { MyProfile } from '@/pages/portal/MyProfile';
import { MyPayslips } from '@/pages/portal/MyPayslips';
import { Leave } from '@/pages/portal/Leave';
import { Claims } from '@/pages/portal/Claims';
import { Notifications } from '@/pages/portal/Notifications';
import { CalendarPage } from '@/pages/calendar/CalendarPage';
import { TeamsPage } from '@/pages/teams/TeamsPage';
import { TeamCalendar } from '@/pages/portal/TeamCalendar';
import { Overtime } from '@/pages/portal/Overtime';
import { CompanyManagement } from '@/pages/admin/CompanyManagement';
import { UserManagement } from '@/pages/admin/UserManagement';
import { RoleManagement } from '@/pages/admin/RoleManagement';
import { PasswordResets } from '@/pages/admin/PasswordResets';
import { LettersPage } from '@/pages/letters/LettersPage';
import { EmployeeImport } from '@/pages/employees/EmployeeImport';
import { BackupPage } from '@/pages/backup/BackupPage';

function RoleGuard({ blockedRoles, children }: { blockedRoles: string[]; children: React.ReactNode }) {
  const { user } = useAuth();
  if (user && blockedRoles.includes(user.role)) {
    return <Navigate to={user.role === 'super_admin' ? '/companies' : '/'} replace />;
  }
  return <>{children}</>;
}

function SuperAdminRedirect({ children }: { children: React.ReactNode }) {
  const { user } = useAuth();
  if (user?.role === 'super_admin') {
    return <Navigate to="/companies" replace />;
  }
  return <>{children}</>;
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
          <Routes>
            <Route path="/login" element={<Login />} />
            <Route path="/forgot-password" element={<ForgotPassword />} />
            <Route path="/reset-password" element={<ResetPassword />} />
            {/* Admin Layout */}
            <Route element={<AppLayout />}>
              <Route path="/" element={<SuperAdminRedirect><Dashboard /></SuperAdminRedirect>} />
              <Route path="/company" element={<CompanyProfile />} />
              <Route path="/employees" element={<EmployeeList />} />
              <Route path="/employees/new" element={<EmployeeCreate />} />
              <Route path="/employees/import" element={<EmployeeImport />} />
              <Route path="/employees/:id" element={<EmployeeDetail />} />
              <Route path="/payroll" element={<RoleGuard blockedRoles={['exec']}><PayrollList /></RoleGuard>} />
              <Route path="/payroll/process" element={<RoleGuard blockedRoles={['exec']}><PayrollProcess /></RoleGuard>} />
              <Route path="/payroll/:id" element={<RoleGuard blockedRoles={['exec']}><PayrollDetail /></RoleGuard>} />
              <Route path="/documents" element={<DocumentList />} />
              <Route path="/calendar" element={<CalendarPage />} />
              <Route path="/teams" element={<TeamsPage />} />
              <Route path="/approvals" element={<Approvals />} />
              <Route path="/reports" element={<Reports />} />
              <Route path="/letters" element={<LettersPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="/companies" element={<RoleGuard blockedRoles={['exec', 'admin', 'payroll_admin', 'hr_manager', 'finance']}><CompanyManagement /></RoleGuard>} />
              <Route path="/users" element={<RoleGuard blockedRoles={['exec', 'admin', 'payroll_admin', 'hr_manager', 'finance']}><UserManagement /></RoleGuard>} />
              <Route path="/roles" element={<RoleGuard blockedRoles={['exec', 'admin', 'payroll_admin', 'hr_manager', 'finance']}><RoleManagement /></RoleGuard>} />
              <Route path="/password-resets" element={<RoleGuard blockedRoles={['exec', 'admin', 'payroll_admin', 'hr_manager', 'finance']}><PasswordResets /></RoleGuard>} />
              <Route path="/backup" element={<RoleGuard blockedRoles={['exec', 'payroll_admin', 'hr_manager', 'finance', 'employee']}><BackupPage /></RoleGuard>} />
            </Route>
            {/* Employee Portal Layout */}
            <Route element={<PortalLayout />}>
              <Route path="/portal" element={<PortalHome />} />
              <Route path="/portal/profile" element={<MyProfile />} />
              <Route path="/portal/payslips" element={<MyPayslips />} />
              <Route path="/portal/leave" element={<Leave />} />
              <Route path="/portal/claims" element={<Claims />} />
              <Route path="/portal/overtime" element={<Overtime />} />
              <Route path="/portal/team-calendar" element={<TeamCalendar />} />
              <Route path="/portal/notifications" element={<Notifications />} />
            </Route>
          </Routes>
        </AuthProvider>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
