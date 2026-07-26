import { Link, useLocation } from 'react-router';
import {
  Building2,
  Users,
  Users2,
  Calculator,
  FileText,
  Settings,
  LogOut,
  ClipboardCheck,
  BarChart3,
  CalendarDays,
  UserCog,
  Shield,
  Mail,
  DatabaseBackup,
  ScrollText,
  X,
  ScanLine,
} from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { CompanySwitcher } from './CompanySwitcher';
import { AnimatePresence, motion } from 'framer-motion';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { hasAnyRole, roleList, type AppRole } from '@/lib/roles';
import { userCan } from '@/lib/usePermissions';
import type { PermissionKey } from '@/api/permissions';

/**
 * `requires` is the permission the destination route enforces, so a link is
 * shown exactly when following it will work. Hand-maintained role lists here had
 * drifted from the routes twice over: `Backup` was offered to `admin`, which the
 * API grants to `super_admin` alone, and `Reports` was offered to `employee`.
 *
 * `hideFor` is a separate, presentational rule and stays role-based: `super_admin`
 * holds every permission but works from the Administration section, so the
 * company workspace links are suppressed for it rather than being unauthorized.
 */
const navigation = [
  { name: 'Company', href: '/company', icon: Building2, hideFor: ['super_admin'], section: 'workspace' },
  { name: 'Employees', href: '/employees', icon: Users, requires: 'view_employees', hideFor: ['super_admin'], section: 'workspace' },
  { name: 'Payroll', href: '/payroll', icon: Calculator, requires: 'view_payroll', section: 'workspace' },
  { name: 'Teams', href: '/teams', icon: Users2, requires: 'view_teams', hideFor: ['super_admin'], section: 'workspace' },
  { name: 'Calendar', href: '/calendar', icon: CalendarDays, requires: 'view_calendar', hideFor: ['super_admin'], section: 'workspace' },
  { name: 'Attendance', href: '/attendance', icon: ScanLine, requires: 'view_attendance', hideFor: ['super_admin'], section: 'workspace' },
  { name: 'Approvals', href: '/approvals', icon: ClipboardCheck, requires: 'view_approvals', hideFor: ['super_admin'], section: 'workspace' },
  // No `hideFor`: super_admin keeps Reports and Payroll, as it did before —
  // these are the two platform-wide entries in the workspace section.
  { name: 'Reports', href: '/reports', icon: BarChart3, requires: 'view_reports', section: 'workspace' },
  { name: 'Documents', href: '/documents', icon: FileText, requires: 'view_documents', hideFor: ['super_admin'], section: 'workspace' },
  { name: 'Letters', href: '/letters', icon: Mail, requires: 'view_email_logs', hideFor: ['super_admin'], section: 'workspace' },
  { name: 'Settings', href: '/settings', icon: Settings, requires: 'manage_company_settings', hideFor: ['super_admin'], section: 'workspace' },
  { name: 'Companies', href: '/companies', icon: Building2, requires: 'manage_companies', section: 'admin' },
  { name: 'Users', href: '/users', icon: UserCog, requires: 'manage_users', section: 'admin' },
  { name: 'Roles', href: '/roles', icon: Shield, requires: 'manage_users', section: 'admin' },
  { name: 'Attendance Settings', href: '/admin/attendance-settings', icon: ScanLine, requires: 'manage_platform_settings', section: 'admin' },
  { name: 'Audit Trail', href: '/audit-trail', icon: ScrollText, requires: 'view_audit_log', section: 'admin' },
  { name: 'Backup', href: '/backup', icon: DatabaseBackup, requires: 'manage_backups', section: 'admin' },
] satisfies ReadonlyArray<{
  name: string;
  href: string;
  icon: typeof Building2;
  requires?: PermissionKey;
  hideFor?: AppRole[];
  section: string;
}>;

const sections = [
  { key: 'workspace', label: 'Workspace' },
  { key: 'admin', label: 'Administration' },
] as const;

interface SidebarProps {
  open?: boolean;
  onClose?: () => void;
}

function SidebarContent({ onClose }: { onClose?: () => void }) {
  const location = useLocation();
  const { user, logout } = useAuth();

  const visibleNav = navigation.filter((item) => {
    if (item.hideFor && hasAnyRole(user, item.hideFor)) return false;
    if (item.requires && !userCan(user, item.requires)) return false;
    return true;
  });

  return (
    <aside className="relative flex flex-col w-64 h-full overflow-hidden bg-slate-950 text-slate-100 border-r border-white/10">
      {/* Aurora glow inside the rail */}
      <div aria-hidden className="pointer-events-none absolute inset-0">
        <div className="absolute -top-24 -left-16 h-64 w-64 rounded-full bg-indigo-600/25 blur-3xl animate-float-a" />
        <div className="absolute top-1/2 -right-24 h-72 w-72 rounded-full bg-violet-600/20 blur-3xl animate-float-b" />
        <div className="absolute -bottom-20 left-6 h-56 w-56 rounded-full bg-fuchsia-600/10 blur-3xl" />
      </div>

      {/* Logo */}
      <div className="relative p-6 border-b border-white/10 flex items-center justify-between">
        <div className="min-w-0">
          <BrandLogo variant="lockup-light" className="h-8 w-auto brightness-125 drop-shadow-md" />
          <span className="mt-2 inline-flex items-center gap-1.5 rounded-full border border-white/10 bg-white/5 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-widest text-indigo-200">
            <span className="glow-dot" />
            Admin Console
          </span>
        </div>
        {onClose && (
          <button
            onClick={onClose}
            className="md:hidden p-2 -mr-2 text-slate-400 hover:text-white hover:bg-white/10 rounded-lg transition-all-fast"
          >
            <X className="w-5 h-5" />
          </button>
        )}
      </div>

      {/* Company Switcher */}
      <div className="relative">
        <CompanySwitcher />
      </div>

      {/* Navigation */}
      <nav className="relative flex-1 py-3 px-3 overflow-y-auto scrollbar-thin">
        {sections.map(({ key, label }) => {
          const items = visibleNav.filter((item) => item.section === key);
          if (items.length === 0) return null;
          return (
            <div key={key}>
              <p className="px-3 pt-3 pb-1.5 text-[10px] font-semibold uppercase tracking-widest text-slate-500">
                {label}
              </p>
              <div className="space-y-0.5">
                {items.map((item) => {
                  const isActive = location.pathname === item.href ||
                    (item.href !== '/' && location.pathname.startsWith(item.href));
                  return (
                    <Link
                      key={item.name}
                      to={item.href}
                      onClick={onClose}
                      className={`group relative flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium transition-all-fast ${
                        isActive ? 'text-white' : 'text-slate-400 hover:text-slate-100 hover:bg-white/5'
                      }`}
                    >
                      {isActive && (
                        <motion.span
                          layoutId="admin-nav-active"
                          className="absolute inset-0 rounded-xl bg-white/10 ring-1 ring-white/15 shadow-[0_0_20px_-6px_var(--glow)]"
                          transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                        />
                      )}
                      {isActive && (
                        <motion.span
                          layoutId="admin-nav-bar"
                          className="absolute left-0 top-1/2 -translate-y-1/2 h-5 w-[3px] rounded-full bg-gradient-to-b from-indigo-400 to-violet-400 shadow-[0_0_10px_var(--glow)]"
                          transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                        />
                      )}
                      <item.icon
                        className={`relative w-[18px] h-[18px] transition-colors ${
                          isActive ? 'text-indigo-300' : 'text-slate-500 group-hover:text-slate-300'
                        }`}
                      />
                      <span className="relative">{item.name}</span>
                    </Link>
                  );
                })}
              </div>
            </div>
          );
        })}
      </nav>

      {/* User */}
      <div
        className="relative p-4 border-t border-white/10 bg-white/[0.03] shrink-0"
        style={{ paddingBottom: 'max(1rem, env(safe-area-inset-bottom))' }}
      >
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-full bg-gradient-to-br from-indigo-500 to-violet-500 ring-2 ring-white/15 shadow-[0_0_16px_-4px_var(--glow)] flex items-center justify-center text-sm font-semibold text-white shrink-0">
            {user?.full_name?.[0] || 'U'}
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-semibold text-white truncate">{user?.full_name || 'User'}</p>
            <p className="text-xs text-slate-400 truncate capitalize">{roleList(user).join(', ').replaceAll('_', ' ')}</p>
          </div>
          <button
            onClick={logout}
            title="Sign Out"
            className="p-2 text-slate-400 hover:text-rose-300 hover:bg-rose-500/10 rounded-lg transition-all-fast shrink-0"
          >
            <LogOut className="w-4 h-4" />
          </button>
        </div>
      </div>
    </aside>
  );
}

export function Sidebar({ open, onClose }: SidebarProps) {
  return (
    <>
      {/* Desktop sidebar */}
      <div className="hidden md:flex">
        <SidebarContent />
      </div>

      {/* Mobile drawer */}
      <AnimatePresence>
        {open && (
          <>
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="fixed inset-0 bg-slate-950/60 backdrop-blur-sm z-40 md:hidden"
              onClick={onClose}
            />
            <motion.div
              initial={{ x: '-100%' }}
              animate={{ x: 0 }}
              exit={{ x: '-100%' }}
              transition={{ type: 'spring', damping: 25, stiffness: 300 }}
              className="fixed inset-y-0 left-0 z-50 md:hidden h-[100dvh]"
            >
              <SidebarContent onClose={onClose} />
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </>
  );
}
