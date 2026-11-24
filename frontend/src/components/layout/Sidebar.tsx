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
  Mail,
  DatabaseBackup,
  ScrollText,
  X,
  ScanLine,
  UserCheck,
  LayoutDashboard,
  Sparkles,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  Bell,
  Download,
  Smartphone,
} from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { usePwa } from '@/context/PwaContext';
import { useTranslation } from 'react-i18next';
import { CompanySwitcher } from './CompanySwitcher';
import { LanguageSwitcher } from '@/components/ui/LanguageSwitcher';
import { AppearanceSwitcher } from '@/components/ui/AppearanceSwitcher';
import { openCommandPalette } from '@/lib/commandPalette';
import { AnimatePresence, motion } from 'framer-motion';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { getAccessToken } from '@/api/client';
import { getDashboardSummary } from '@/api/dashboard';
import { hasAnyRole, roleList, type AppRole } from '@/lib/roles';
import { userCan } from '@/lib/usePermissions';
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
  { nameKey: 'nav.dashboard', href: '/dashboard', icon: LayoutDashboard, hideFor: ['super_admin'], section: 'workspace' },
  { nameKey: 'nav.company', href: '/company', icon: Building2, hideFor: ['super_admin'], section: 'workspace' },
  { nameKey: 'nav.employees', href: '/employees', icon: Users, requires: 'view_employees', hideFor: ['super_admin'], section: 'workspace' },
  { nameKey: 'nav.payroll', href: '/payroll', icon: Calculator, requires: 'view_payroll', section: 'workspace' },
  { nameKey: 'nav.teams', href: '/teams', icon: Users2, requires: 'view_teams', hideFor: ['super_admin'], section: 'workspace' },
  { nameKey: 'nav.calendar', href: '/calendar', icon: CalendarDays, requires: 'view_calendar', hideFor: ['super_admin'], section: 'workspace' },
  { nameKey: 'nav.attendance', href: '/attendance', icon: ScanLine, requires: 'view_attendance', hideFor: ['super_admin'], section: 'workspace' },
  { nameKey: 'nav.approvals', href: '/approvals', icon: ClipboardCheck, requires: 'view_approvals', hideFor: ['super_admin'], section: 'workspace' },
  { nameKey: 'nav.reports', href: '/reports', icon: BarChart3, requires: 'view_reports', section: 'workspace' },
  { nameKey: 'nav.documents', href: '/documents', icon: FileText, requires: 'view_documents', hideFor: ['super_admin'], section: 'workspace' },
  { nameKey: 'nav.letters', href: '/letters', icon: Mail, requires: 'view_email_logs', hideFor: ['super_admin'], section: 'workspace' },
  { nameKey: 'nav.settings', href: '/settings', icon: Settings, requires: 'manage_company_settings', hideFor: ['super_admin'], section: 'workspace' },
  { nameKey: 'nav.myAttendance', href: '/my/attendance', icon: UserCheck, requiresEmployeeProfile: true, section: 'personal' },
  { nameKey: 'nav.myPortal', href: '/portal/profile', icon: Sparkles, requiresEmployeeProfile: true, section: 'personal' },
  { nameKey: 'nav.companies', href: '/companies', icon: Building2, requires: 'manage_companies', section: 'admin' },
  { nameKey: 'nav.users', href: '/users', icon: UserCog, requires: 'manage_users', section: 'admin' },
  { nameKey: 'nav.roles', href: '/roles', icon: Shield, requires: 'manage_users', section: 'admin' },
  { nameKey: 'nav.userGroups', href: '/user-groups', icon: Users2, requires: 'manage_users', section: 'admin' },
  { nameKey: 'nav.attendanceSettings', href: '/admin/attendance-settings', icon: ScanLine, requires: 'manage_platform_settings', section: 'admin' },
  { nameKey: 'nav.auditTrail', href: '/audit-trail', icon: ScrollText, requires: 'view_audit_log', section: 'admin' },
  { nameKey: 'nav.backup', href: '/backup', icon: DatabaseBackup, requires: 'manage_backups', section: 'admin' },
] satisfies ReadonlyArray<{
  nameKey: string;
  href: string;
  icon: typeof Building2;
  requires?: PermissionKey;
  hideFor?: AppRole[];
  requiresEmployeeProfile?: boolean;
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
}: {
  onClose?: () => void;
  collapsed?: boolean;
  onToggleCollapse?: () => void;
}) {
  const location = useLocation();
  const { user, logout } = useAuth();
  const {
    canPromptInstall,
    canShowInstallGuide,
    notificationStatus,
    notificationBusy,
    triggerInstall,
    enableNotifications,
  } = usePwa();
  const { t } = useTranslation();

  const visibleNav = navigation.filter((item) => {
    if (item.hideFor && hasAnyRole(user, item.hideFor)) return false;
    if (item.requires && !userCan(user, item.requires)) return false;
    if (item.requiresEmployeeProfile && !user?.employee_id) return false;
    return true;
  });

  const canSeeApprovalsBadge =
    Boolean(getAccessToken()) &&
    Boolean(user?.company_id) &&
    !user?.must_change_password &&
    userCan(user, 'view_approvals') &&
    !hasAnyRole(user, ['super_admin']);

  const { data: dashSummary } = useQuery({
    queryKey: ['dashboard-summary', 7],
    queryFn: () => getDashboardSummary(7),
    enabled: canSeeApprovalsBadge,
    staleTime: 60_000,
    refetchInterval: 60_000,
  });

  const pendingApprovals = dashSummary
    ? dashSummary.needs_attention.pending_leave +
      dashSummary.needs_attention.pending_claims +
      dashSummary.needs_attention.pending_overtime
    : 0;

  return (
    <aside
      className={`relative flex flex-col h-full overflow-hidden bg-slate-950 text-slate-100 border-r border-white/10 transition-[width] duration-200 ${
        collapsed ? 'w-[76px]' : 'w-64'
      }`}
    >
      {/* Aurora glow inside the rail */}
      <div aria-hidden className="pointer-events-none absolute inset-0">
        <div className="absolute -top-24 -left-16 h-64 w-64 rounded-full bg-indigo-600/25 blur-3xl animate-float-a" />
        <div className="absolute top-1/2 -right-24 h-72 w-72 rounded-full bg-violet-600/20 blur-3xl animate-float-b" />
        <div className="absolute -bottom-20 left-6 h-56 w-56 rounded-full bg-fuchsia-600/10 blur-3xl" />
      </div>

      {/* Logo + collapse / close */}
      <div className={`relative border-b border-white/10 flex items-center justify-between ${collapsed ? 'p-3.5 justify-center' : 'p-5'}`}>
        {!collapsed && (
          <div className="min-w-0">
            <BrandLogo variant="lockup-light" className="h-8 w-auto brightness-125 drop-shadow-md" />
            <span className="mt-2 inline-flex items-center gap-1.5 rounded-full border border-white/10 bg-white/5 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-widest text-indigo-200">
              <span className="glow-dot" />
              {t('nav.adminConsoleBadge')}
            </span>
          </div>
        )}
        {onToggleCollapse && !onClose && (
          <button
            type="button"
            onClick={onToggleCollapse}
            title={collapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')}
            aria-label={collapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')}
            className="hidden md:inline-flex p-2 text-slate-400 hover:text-white hover:bg-white/10 rounded-lg transition-all-fast shrink-0"
          >
            {collapsed ? <PanelLeftOpen className="w-4 h-4" /> : <PanelLeftClose className="w-4 h-4" />}
          </button>
        )}
        {onClose && (
          <button
            onClick={onClose}
            aria-label={t('a11y.closeMenu')}
            className="md:hidden p-2 -mr-2 text-slate-400 hover:text-white hover:bg-white/10 rounded-lg transition-all-fast"
          >
            <X className="w-5 h-5" />
          </button>
        )}
      </div>

      {/* Company Switcher + Quick Search */}
      {!collapsed && (
        <div className="relative">
          <CompanySwitcher />
          <div className="px-3 pt-2">
            <button
              type="button"
              onClick={openCommandPalette}
              className="flex w-full items-center justify-between rounded-xl border border-white/10 bg-white/[0.04] px-3 py-2 text-xs text-slate-400 transition-all hover:border-indigo-400/30 hover:bg-white/[0.08] hover:text-slate-200"
            >
              <span className="inline-flex items-center gap-2 truncate">
                <Search className="h-3.5 w-3.5 shrink-0 text-indigo-300" aria-hidden="true" />
                <span className="truncate">{t('nav.quickSearch')}</span>
              </span>
              <kbd className="rounded border border-white/10 bg-white/5 px-1.5 py-0.5 text-[10px] font-semibold text-slate-400">
                ⌘K
              </kbd>
            </button>
          </div>
        </div>
      )}

      {/* Navigation */}
      <nav className="relative flex-1 py-3 px-3 overflow-y-auto scrollbar-thin">
        {sections.map(({ key, labelKey }) => {
          const items = visibleNav.filter((item) => item.section === key);
          if (items.length === 0) return null;
          return (
            <div key={key}>
              <p
                className={`px-3 pt-3 pb-1.5 text-[10px] font-semibold uppercase tracking-widest text-slate-500 ${
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
                      location.pathname.startsWith(item.href));
                  const label = t(item.nameKey);
                  return (
                    <Link
                      key={item.nameKey}
                      to={item.href}
                      onClick={onClose}
                      title={collapsed ? label : undefined}
                      className={`group relative flex items-center ${
                        collapsed ? 'justify-center px-2 py-2.5' : 'gap-3 px-3 py-2.5'
                      } rounded-xl text-sm font-medium transition-all-fast ${
                        isActive ? 'text-white' : 'text-slate-400 hover:text-slate-100 hover:bg-white/5'
                      }`}
                    >
                      {isActive && (
                        <motion.span
                          layoutId={onClose ? 'admin-nav-active-mobile' : 'admin-nav-active'}
                          className="absolute inset-0 rounded-xl bg-white/10 ring-1 ring-white/15 shadow-[0_0_20px_-6px_var(--glow)]"
                          transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                        />
                      )}
                      {isActive && (
                        <motion.span
                          layoutId={onClose ? 'admin-nav-bar-mobile' : 'admin-nav-bar'}
                          className="absolute left-0 top-1/2 -translate-y-1/2 h-5 w-[3px] rounded-full bg-gradient-to-b from-indigo-400 to-violet-400 shadow-[0_0_10px_var(--glow)]"
                          transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                        />
                      )}
                      <item.icon
                        className={`relative w-[18px] h-[18px] shrink-0 transition-colors ${
                          isActive ? 'text-indigo-300' : 'text-slate-500 group-hover:text-slate-300'
                        }`}
                      />
                      <span className={collapsed ? 'sr-only' : 'relative truncate flex-1'}>{label}</span>
                      {item.href === '/approvals' && pendingApprovals > 0 && (
                        <span
                          aria-hidden="true"
                          data-badge={pendingApprovals > 99 ? '99+' : String(pendingApprovals)}
                          className={
                            collapsed
                              ? 'absolute right-2 top-2 h-2 w-2 rounded-full bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.7)]'
                              : 'relative ml-auto inline-flex min-w-[20px] h-5 items-center justify-center rounded-full bg-amber-500/20 px-1.5 text-[10px] font-bold text-amber-300 ring-1 ring-amber-400/30 after:content-[attr(data-badge)]'
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

      {/* User & Preferences */}
      <div
        className="relative p-4 border-t border-white/10 bg-white/[0.03] shrink-0"
        style={{ paddingBottom: 'max(1rem, env(safe-area-inset-bottom))' }}
      >
        {!collapsed && (
          <div className="mb-3 space-y-2">
            {(canShowInstallGuide ||
              notificationStatus === 'default' ||
              notificationStatus === 'ios-install-required') && (
              <div className="space-y-1.5">
                {canShowInstallGuide && (
                  <button
                    type="button"
                    onClick={() => {
                      onClose?.();
                      void triggerInstall();
                    }}
                    className="flex w-full items-center gap-2 rounded-lg border border-white/15 bg-white/5 px-2.5 py-1.5 text-xs font-medium text-slate-200 hover:bg-white/10 transition-colors"
                  >
                    {canPromptInstall ? (
                      <Download className="w-3.5 h-3.5 shrink-0 text-indigo-300" aria-hidden="true" />
                    ) : (
                      <Smartphone className="w-3.5 h-3.5 shrink-0 text-indigo-300" aria-hidden="true" />
                    )}
                    <span className="truncate">
                      {canPromptInstall
                        ? t('pwa.actions.installApp')
                        : t('pwa.actions.addToHomeScreen')}
                    </span>
                  </button>
                )}
                {(notificationStatus === 'default' ||
                  notificationStatus === 'ios-install-required') && (
                  <button
                    type="button"
                    disabled={notificationBusy}
                    onClick={() => {
                      onClose?.();
                      void enableNotifications();
                    }}
                    className="flex w-full items-center gap-2 rounded-lg border border-white/15 bg-white/5 px-2.5 py-1.5 text-xs font-medium text-slate-200 hover:bg-white/10 transition-colors disabled:opacity-50"
                  >
                    <Bell className="w-3.5 h-3.5 shrink-0 text-indigo-300" aria-hidden="true" />
                    <span className="truncate">{t('pwa.actions.allowNotifications')}</span>
                  </button>
                )}
              </div>
            )}
            <AppearanceSwitcher darkRail />
            <LanguageSwitcher dark />
            <div className="flex flex-wrap items-center justify-center gap-2 pt-1 text-[11px] text-slate-400">
              <Link to="/terms" onClick={onClose} className="hover:text-slate-200 transition-colors">
                {t('legal.links.terms')}
              </Link>
              <span className="text-slate-600">·</span>
              <Link to="/privacy" onClick={onClose} className="hover:text-slate-200 transition-colors">
                {t('legal.links.privacy')}
              </Link>
              <span className="text-slate-600">·</span>
              <Link to="/compliance" onClick={onClose} className="hover:text-slate-200 transition-colors">
                {t('legal.links.compliance')}
              </Link>
            </div>
          </div>
        )}
        <div className={`flex items-center ${collapsed ? 'flex-col gap-2' : 'gap-3'}`}>
          <div className="w-9 h-9 rounded-full bg-gradient-to-br from-indigo-500 to-violet-500 ring-2 ring-white/15 shadow-[0_0_16px_-4px_var(--glow)] flex items-center justify-center text-sm font-semibold text-white shrink-0">
            {user?.full_name?.[0] || 'U'}
          </div>
          <div className={collapsed ? 'sr-only' : 'flex-1 min-w-0'}>
            <p className="text-sm font-semibold text-white truncate">{user?.full_name || t('common.user')}</p>
            <p className="text-xs text-slate-400 truncate capitalize">
              {roleList(user).map((role) => t(`roles.${role}`)).join(', ')}
            </p>
          </div>
          <button
            onClick={logout}
            title={t('nav.signOut')}
            aria-label={t('nav.signOut')}
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
  const [collapsed, setCollapsed] = useState<boolean>(() => readCollapsed());

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
      <div className="hidden md:flex">
        <SidebarContent collapsed={collapsed} onToggleCollapse={toggleCollapse} />
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
