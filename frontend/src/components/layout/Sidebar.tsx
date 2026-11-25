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
import { BrandLogo } from '@/components/ui/BrandLogo';
import { getAccessToken } from '@/api/client';
import { getPendingApprovalCounts } from '@/api/approvals';
import { getNotificationCount } from '@/api/notifications';
import { hasAnyRole, roleList, type AppRole } from '@/lib/roles';
import { userCan, userCanAny } from '@/lib/usePermissions';
import { useMediaQuery } from '@/lib/useMediaQuery';
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
  { nameKey: 'nav.myPortal', href: '/portal/profile', icon: Sparkles, requiresEmployeeProfile: true, section: 'personal' },
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
  const {
    canPromptInstall,
    canShowInstallGuide,
    notificationStatus,
    notificationBusy,
    triggerInstall,
    enableNotifications,
  } = usePwa();
  const { t } = useTranslation();
  const { data: notifCount } = useQuery({
    queryKey: ['notification-count'],
    queryFn: getNotificationCount,
    refetchInterval: 30_000,
    enabled: Boolean(user?.company_id) && !user?.must_change_password,
  });
  const unreadNotifications = notifCount?.unread ?? 0;

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
      className={`relative flex flex-col h-full overflow-hidden bg-slate-950 text-slate-100 border-r border-white/10 transition-[width] duration-200 ${
        collapsed ? 'w-[76px]' : 'w-64'
      }`}
    >
      {/* Aurora glow inside the rail */}
      <div aria-hidden className="aurora-rail pointer-events-none absolute inset-0" />

      {/* Logo + collapse / close */}
      <div className={`relative border-b border-white/10 flex items-center justify-between ${collapsed ? 'p-3.5 justify-center' : 'p-5'}`}>
        {!collapsed && (
          <div className="min-w-0">
            <BrandLogo variant="lockup-dark" priority className="h-8 w-auto rounded-lg drop-shadow-md" />
            <span className="mt-2 inline-flex items-center gap-1.5 rounded-full border border-white/10 bg-white/5 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-widest text-indigo-200">
              <span className="glow-dot" />
              {t('nav.adminConsoleBadge')}
            </span>
          </div>
        )}
        <div className="flex items-center gap-1 shrink-0">
          {!collapsed && user?.company_id && (
            <Link
              to="/portal/notifications"
              onClick={onClose}
              title={t('nav.portal.notifications')}
              aria-label={t('nav.portal.notifications')}
              className="relative hidden md:inline-flex p-2 text-slate-400 hover:text-white hover:bg-white/10 rounded-lg transition-all-fast"
            >
              <Bell className="w-4 h-4" />
              {unreadNotifications > 0 && (
                <span className="absolute top-1 right-1 min-w-[15px] h-[15px] px-0.5 text-[9px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none">
                  {unreadNotifications > 99 ? '99+' : unreadNotifications}
                </span>
              )}
            </Link>
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
              type="button"
              onClick={onClose}
              aria-label={t('a11y.closeMenu')}
              className="md:hidden min-h-11 min-w-11 inline-flex items-center justify-center -mr-2 text-slate-400 hover:text-white hover:bg-white/10 rounded-lg transition-all-fast"
            >
              <X className="w-5 h-5" />
            </button>
          )}
        </div>
      </div>

      {/* Company Switcher + Quick Search */}
      {collapsed ? (
        <div className="px-2.5 pt-2">
          <button
            type="button"
            onClick={openCommandPalette}
            title={`${t('nav.quickSearch')} (⌘K)`}
            aria-label={t('nav.quickSearch')}
            className="flex w-full min-h-10 items-center justify-center rounded-xl border border-white/10 bg-white/[0.04] p-2 text-slate-400 transition-all hover:border-indigo-400/30 hover:bg-white/[0.08] hover:text-slate-200"
          >
            <Search className="h-4 w-4 shrink-0 text-indigo-300" aria-hidden="true" />
          </button>
        </div>
      ) : (
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
                      location.pathname.startsWith(`${item.href}/`));
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
                        <span className="absolute inset-0 rounded-xl bg-white/10 ring-1 ring-white/15 shadow-[0_0_20px_-6px_var(--glow)]" />
                      )}
                      {isActive && (
                        <span className="absolute left-0 top-1/2 -translate-y-1/2 h-5 w-[3px] rounded-full bg-gradient-to-b from-indigo-400 to-violet-400 shadow-[0_0_10px_var(--glow)]" />
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
        <div className={`flex items-center ${collapsed ? 'flex-col gap-2' : 'gap-2'}`}>
          <Link
            to="/account"
            onClick={onClose}
            title={t('nav.accountSecurity')}
            aria-label={collapsed ? t('nav.accountSecurity') : undefined}
            className={`group flex items-center min-w-0 rounded-xl transition-colors hover:bg-white/10 ${
              collapsed ? 'p-1.5 justify-center' : 'flex-1 gap-3 p-1.5 -m-1.5'
            }`}
          >
            <div className="w-9 h-9 rounded-full bg-gradient-to-br from-indigo-500 to-violet-500 ring-2 ring-white/15 shadow-[0_0_16px_-4px_var(--glow)] flex items-center justify-center text-sm font-semibold text-white shrink-0">
              {user?.full_name?.[0] || 'U'}
            </div>
            <div className={collapsed ? 'sr-only' : 'flex-1 min-w-0'}>
              <p className="text-sm font-semibold text-white truncate group-hover:text-indigo-200 transition-colors">
                {user?.full_name || t('common.user')}
              </p>
              <p className="text-xs text-slate-400 truncate capitalize">
                {roleList(user).map((role) => t(`roles.${role}`)).join(', ')}
              </p>
            </div>
          </Link>
          {!collapsed && (
            <Link
              to="/account"
              onClick={onClose}
              title={t('nav.accountSecurity')}
              aria-label={t('nav.accountSecurity')}
              className={`p-2 rounded-lg transition-all-fast shrink-0 ${
                location.pathname === '/account'
                  ? 'text-indigo-300 bg-white/10'
                  : 'text-slate-400 hover:text-white hover:bg-white/10'
              }`}
            >
              <Shield className="w-4 h-4" />
            </Link>
          )}
          <button
            type="button"
            onClick={() => void logout()}
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
            className="animate-fade-in fixed inset-0 bg-slate-950/60 backdrop-blur-sm z-40 md:hidden"
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
