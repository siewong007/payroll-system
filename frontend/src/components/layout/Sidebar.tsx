import { useState } from 'react';
import { Link, useLocation } from 'react-router';
import { useQuery } from '@tanstack/react-query';
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
  ShieldCheck,
  Mail,
  DatabaseBackup,
  ScrollText,
  X,
  ScanLine,
  UserCheck,
  LayoutDashboard,
  UserCircle,
  PanelLeftClose,
  PanelLeftOpen,
} from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { useTranslation } from 'react-i18next';
import { CompanySwitcher } from './CompanySwitcher';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { getAccessToken } from '@/api/client';
import { getPendingApprovalCounts } from '@/api/approvals';
import { hasAnyRole, roleList, type AppRole } from '@/lib/roles';
import { userCan, userCanAny } from '@/lib/usePermissions';
import { useMediaQuery } from '@/lib/useMediaQuery';
import { prefetchRoute } from '@/lib/routePrefetch';
import type { PermissionKey } from '@/api/permissions';

const SIDEBAR_COLLAPSED_KEY = 'payroll-sidebar-collapsed';

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === 'true';
  } catch {
    return false;
  }
}

const navigation = [
  { nameKey: 'nav.dashboard', href: '/dashboard', icon: LayoutDashboard, requires: 'view_employees', hideFor: ['super_admin'], requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.company', href: '/company', icon: Building2, hideFor: ['super_admin'], requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.employees', href: '/employees', icon: Users, requires: 'view_employees', hideFor: ['super_admin'], requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.payroll', href: '/payroll', icon: Calculator, requires: 'view_payroll', requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.teams', href: '/teams', icon: Users2, requiresAny: ['view_teams', 'manage_teams'], hideFor: ['super_admin'], requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.calendar', href: '/calendar', icon: CalendarDays, requires: 'view_calendar', hideFor: ['super_admin'], requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.attendance', href: '/attendance', icon: ScanLine, requires: 'view_attendance', hideFor: ['super_admin'], requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.approvals', href: '/approvals', icon: ClipboardCheck, requires: 'view_approvals', hideFor: ['super_admin'], requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.reports', href: '/reports', icon: BarChart3, requires: 'view_reports', requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.documents', href: '/documents', icon: FileText, requires: 'view_documents', hideFor: ['super_admin'], requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.letters', href: '/letters', icon: Mail, requiresAny: ['view_email_logs', 'manage_email_templates', 'send_letters'], hideFor: ['super_admin'], requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.settings', href: '/settings', icon: Settings, requires: 'manage_company_settings', hideFor: ['super_admin'], requiresCompany: true, section: 'workspace' },
  { nameKey: 'nav.myAttendance', href: '/my/attendance', icon: UserCheck, requiresEmployeeProfile: true, section: 'personal' },
  { nameKey: 'nav.myPortal', href: '/portal/profile', icon: UserCircle, requiresEmployeeProfile: true, section: 'personal' },
  { nameKey: 'nav.companies', href: '/companies', icon: Building2, requires: 'manage_companies', section: 'admin' },
  { nameKey: 'nav.users', href: '/users', icon: UserCog, requires: 'manage_users', section: 'admin' },
  { nameKey: 'nav.roles', href: '/roles', icon: Shield, requires: 'view_user_directory', section: 'admin' },
  { nameKey: 'nav.userGroups', href: '/user-groups', icon: Users2, requires: 'view_user_directory', requiresCompany: true, section: 'admin' },
  { nameKey: 'nav.attendanceSettings', href: '/admin/attendance-settings', icon: ScanLine, requires: 'manage_platform_settings', section: 'admin' },
  { nameKey: 'nav.platformSettings', href: '/admin/platform', icon: ShieldCheck, requires: 'manage_platform_settings', section: 'admin' },
  { nameKey: 'nav.auditTrail', href: '/audit-trail', icon: ScrollText, requires: 'view_audit_log', requiresCompany: true, section: 'admin' },
  { nameKey: 'nav.backup', href: '/backup', icon: DatabaseBackup, requires: 'manage_backups', section: 'admin' },
] satisfies ReadonlyArray<{
  nameKey: string;
  href: string;
  icon: typeof Building2;
  requires?: PermissionKey;
  requiresAny?: PermissionKey[];
  hideFor?: AppRole[];
  requiresEmployeeProfile?: boolean;
  requiresCompany?: boolean;
  section: string;
}>;

const sections = [
  { key: 'workspace', labelKey: 'nav.section.workspace' },
  { key: 'personal', labelKey: 'nav.section.personal' },
  { key: 'admin', labelKey: 'nav.section.admin' },
] as const;

interface SidebarProps {
  open?: boolean;
  onClose?: () => void;
}

function SidebarContent({
  onClose,
  collapsed = false,
  onToggleCollapse,
  pendingApprovals = 0,
}: {
  onClose?: () => void;
  collapsed?: boolean;
  onToggleCollapse?: () => void;
  pendingApprovals?: number;
}) {
  const location = useLocation();
  const { user, logout } = useAuth();
  const { t } = useTranslation();

  const visibleNav = navigation.filter((item) => {
    if (item.hideFor && hasAnyRole(user, item.hideFor)) return false;
    if (item.requires && !userCan(user, item.requires)) return false;
    if (item.requiresAny && !userCanAny(user, item.requiresAny)) return false;
    if (item.requiresEmployeeProfile && !user?.employee_id) return false;
    if (item.requiresCompany && !user?.company_id) return false;
    return true;
  });

  return (
    <aside
      className={`relative flex flex-col h-full overflow-hidden bg-[var(--sidebar)] text-[var(--foreground)] border-r border-[var(--border)] transition-[width] duration-150 ${
        collapsed ? 'w-16' : 'w-56'
      }`}
    >
      {/* Logo + collapse / close aligned with top header height */}
      <div
        className={`relative h-12 border-b border-[var(--border)] flex items-center shrink-0 ${
          collapsed ? 'px-2 justify-center' : 'px-3.5 justify-between'
        }`}
      >
        {!collapsed && (
          <BrandLogo variant="lockup-auto" priority className="h-6 w-auto" />
        )}
        <div className="flex items-center gap-1 shrink-0">
          {onToggleCollapse && !onClose && (
            <button
              type="button"
              onClick={onToggleCollapse}
              title={collapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')}
              aria-label={collapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')}
              className="hidden md:inline-flex p-1.5 text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-black/[0.04] dark:hover:bg-white/[0.06] rounded-md transition-all-fast shrink-0"
            >
              {collapsed ? <PanelLeftOpen className="w-4 h-4" /> : <PanelLeftClose className="w-4 h-4" />}
            </button>
          )}
          {onClose && (
            <button
              type="button"
              onClick={onClose}
              aria-label={t('a11y.closeMenu')}
              className="md:hidden min-h-11 min-w-11 inline-flex items-center justify-center -mr-1.5 text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-black/[0.04] dark:hover:bg-white/[0.06] rounded-md transition-all-fast"
            >
              <X className="w-5 h-5" />
            </button>
          )}
        </div>
      </div>

      {/* Company Switcher */}
      {!collapsed && (
        <div className="relative">
          <CompanySwitcher />
        </div>
      )}

      {/* Navigation */}
      <nav className="relative flex-1 py-2.5 px-2.5 overflow-y-auto scrollbar-thin">
        {sections.map(({ key, labelKey }) => {
          const items = visibleNav.filter((item) => item.section === key);
          if (items.length === 0) return null;
          return (
            <div key={key}>
              <p
                className={`px-2 pt-3 pb-1 text-[10px] font-medium uppercase tracking-[0.06em] text-[var(--subtle-foreground)] ${
                  collapsed ? 'sr-only' : ''
                }`}
              >
                {t(labelKey)}
              </p>
              <div className="space-y-0.5">
                {items.map((item) => {
                  const isActive =
                    location.pathname === item.href ||
                    (item.href !== '/' &&
                      !item.href.startsWith('/portal') &&
                      location.pathname.startsWith(`${item.href}/`));
                  const label = t(item.nameKey);
                  return (
                    <Link
                      key={item.nameKey}
                      to={item.href}
                      onClick={onClose}
                      onMouseEnter={() => prefetchRoute(item.href)}
                      onFocus={() => prefetchRoute(item.href)}
                      title={collapsed ? label : undefined}
                      aria-current={isActive ? 'page' : undefined}
                      className={`group relative flex items-center ${
                        collapsed ? 'justify-center px-2 py-2' : 'gap-2.5 px-2.5 py-1.5'
                      } rounded-md text-[13px] transition-all-fast ${
                        isActive
                          ? 'bg-[var(--card)] text-[var(--foreground)] font-medium border border-[var(--border)] shadow-[0_1px_2px_rgba(0,0,0,0.03)]'
                          : 'text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-black/[0.04] dark:hover:bg-white/[0.05] border border-transparent'
                      }`}
                    >
                      <item.icon
                        className={`w-4 h-4 shrink-0 ${
                          isActive ? 'text-[var(--accent-1)]' : 'text-[var(--subtle-foreground)] group-hover:text-[var(--foreground)]'
                        }`}
                      />
                      <span className={collapsed ? 'sr-only' : 'truncate flex-1'}>{label}</span>
                      {item.href === '/approvals' && pendingApprovals > 0 && (
                        <span
                          aria-hidden="true"
                          data-badge={pendingApprovals > 99 ? '99+' : String(pendingApprovals)}
                          className={
                            collapsed
                              ? 'absolute right-1.5 top-1.5 h-1.5 w-1.5 rounded-full bg-amber-500'
                              : 'ml-auto inline-flex min-w-[18px] h-4 items-center justify-center rounded-full bg-amber-500/15 border border-amber-500/25 px-1 text-[10px] font-semibold text-amber-700 dark:text-amber-300 tabular-nums after:content-[attr(data-badge)]'
                          }
                        />
                      )}
                    </Link>
                  );
                })}
              </div>
            </div>
          );
        })}
      </nav>

      {/* Compact User Footer */}
      <div
        className="relative p-2.5 border-t border-[var(--border)] bg-[var(--sidebar)] shrink-0"
        style={{ paddingBottom: 'max(0.625rem, env(safe-area-inset-bottom))' }}
      >
        <div className={`flex items-center ${collapsed ? 'flex-col gap-1.5' : 'gap-1.5'}`}>
          <Link
            to="/account"
            onClick={onClose}
            onMouseEnter={() => prefetchRoute('/account')}
            title={t('nav.accountSecurity')}
            aria-label={collapsed ? t('nav.accountSecurity') : undefined}
            className={`group flex items-center min-w-0 rounded-md transition-colors ${
              collapsed ? 'p-1.5 justify-center' : 'flex-1 gap-2.5 px-2 py-1.5'
            } ${
              location.pathname === '/account'
                ? 'bg-[var(--card)] border border-[var(--border)]'
                : 'hover:bg-black/[0.04] dark:hover:bg-white/[0.05] border border-transparent'
            }`}
          >
            <div className="w-7 h-7 rounded-md bg-[var(--accent-1)] text-[var(--accent-fg)] flex items-center justify-center text-xs font-semibold shrink-0">
              {user?.full_name?.[0] || 'U'}
            </div>
            <div className={collapsed ? 'sr-only' : 'flex-1 min-w-0'}>
              <p className="text-xs font-medium text-[var(--foreground)] truncate">
                {user?.full_name || t('common.user')}
              </p>
              <p className="text-[11px] text-[var(--subtle-foreground)] truncate capitalize">
                {roleList(user).map((role) => t(`roles.${role}`)).join(', ')}
              </p>
            </div>
          </Link>
          <button
            type="button"
            onClick={() => void logout()}
            title={t('nav.signOut')}
            aria-label={t('nav.signOut')}
            className="p-1.5 text-[var(--muted-foreground)] hover:text-red-600 dark:hover:text-red-400 hover:bg-red-500/10 rounded-md transition-all-fast shrink-0"
          >
            <LogOut className="w-4 h-4" />
          </button>
        </div>
      </div>
    </aside>
  );
}

export function Sidebar({ open, onClose }: SidebarProps) {
  const { user } = useAuth();
  const isDesktop = useMediaQuery('(min-width: 768px)', true);
  const [collapsed, setCollapsed] = useState<boolean>(() => readCollapsed());

  const canSeeApprovalsBadge =
    Boolean(getAccessToken()) &&
    Boolean(user?.company_id) &&
    !user?.must_change_password &&
    userCan(user, 'view_approvals') &&
    !hasAnyRole(user, ['super_admin']);

  const { data: approvalCounts } = useQuery({
    queryKey: ['approvals-pending-count'],
    queryFn: getPendingApprovalCounts,
    enabled: canSeeApprovalsBadge,
    staleTime: 60_000,
    refetchInterval: 60_000,
  });

  const pendingApprovals = approvalCounts?.total ?? 0;

  const toggleCollapse = () => {
    setCollapsed((prev) => {
      const next = !prev;
      try {
        localStorage.setItem(SIDEBAR_COLLAPSED_KEY, String(next));
      } catch {
        // Ignore storage errors.
      }
      return next;
    });
  };

  return (
    <>
      {/* Desktop sidebar */}
      {isDesktop && (
        <div className="hidden md:flex">
          <SidebarContent
            collapsed={collapsed}
            onToggleCollapse={toggleCollapse}
            pendingApprovals={pendingApprovals}
          />
        </div>
      )}

      {/* Mobile drawer */}
      {open && (
        <>
          <div
            className="animate-fade-in fixed inset-0 bg-black/45 z-40 md:hidden"
            onClick={onClose}
          />
          <div className="animate-slide-in-left fixed inset-y-0 left-0 z-50 md:hidden h-[100dvh]">
            <SidebarContent onClose={onClose} pendingApprovals={pendingApprovals} />
          </div>
        </>
      )}
    </>
  );
}
