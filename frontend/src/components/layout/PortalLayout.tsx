import { Outlet, Link, useLocation, Navigate } from 'react-router';
import { User, FileText, Calendar, Receipt, LogOut, ChevronDown, Bell, Users, Clock, MoreHorizontal, ScanLine, LayoutDashboard, Search, Download, Smartphone, Info } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { usePwa } from '@/context/PwaContext';
import { canUseAdminConsole } from '@/lib/roles';
import { useEffect, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getNotificationCount } from '@/api/notifications';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { LanguageSwitcher } from '@/components/ui/LanguageSwitcher';
import { AppearanceSwitcher } from '@/components/ui/AppearanceSwitcher';
import { PwaInstallBanner } from '@/components/pwa/PwaInstallBanner';
import { openCommandPalette } from '@/lib/commandPalette';
import { prefetchRoute } from '@/lib/routePrefetch';
import { PageTransition } from '@/components/ui/PageTransition';
import { useTranslation } from 'react-i18next';

const portalNav = [
  { nameKey: 'nav.portal.profile', href: '/portal/profile', icon: User },
  { nameKey: 'nav.portal.payslips', href: '/portal/payslips', icon: FileText },
  { nameKey: 'nav.portal.leave', href: '/portal/leave', icon: Calendar },
  { nameKey: 'nav.portal.attendance', href: '/portal/attendance', icon: ScanLine },
  { nameKey: 'nav.portal.claims', href: '/portal/claims', icon: Receipt },
  { nameKey: 'nav.portal.overtime', href: '/portal/overtime', icon: Clock },
  { nameKey: 'nav.portal.teamCalendar', href: '/portal/team-calendar', icon: Users },
];

// Compact desktop nav for md..lg viewports before xl fits all 7 links
const compactDesktopPrimaryNav = [
  { nameKey: 'nav.portal.profileShort', href: '/portal/profile', icon: User },
  { nameKey: 'nav.portal.payslipsShort', href: '/portal/payslips', icon: FileText },
  { nameKey: 'nav.portal.leave', href: '/portal/leave', icon: Calendar },
  { nameKey: 'nav.portal.attendance', href: '/portal/attendance', icon: ScanLine },
];
const compactDesktopMoreNav = portalNav.slice(4);

// Primary tabs shown in mobile bottom bar
const mobileTabNav = [
  { nameKey: 'nav.portal.profileShort', href: '/portal/profile', icon: User },
  { nameKey: 'nav.portal.payslipsShort', href: '/portal/payslips', icon: FileText },
  { nameKey: 'nav.portal.leave', href: '/portal/leave', icon: Calendar },
  { nameKey: 'nav.portal.attendance', href: '/portal/attendance', icon: ScanLine },
];

// Items shown in "More" menu on mobile
const mobileMoreNav = [
  { nameKey: 'nav.portal.claims', href: '/portal/claims', icon: Receipt },
  { nameKey: 'nav.portal.overtime', href: '/portal/overtime', icon: Clock },
  { nameKey: 'nav.portal.teamCalendar', href: '/portal/team-calendar', icon: Users },
  { nameKey: 'nav.portal.notifications', href: '/portal/notifications', icon: Bell },
];

export function PortalLayout() {
  const { user, logout, isAuthenticated, isLoading } = useAuth();
  const {
    canPromptInstall,
    canShowInstallGuide,
    notificationStatus,
    notificationBusy,
    triggerInstall,
    enableNotifications,
  } = usePwa();
  const { t } = useTranslation();
  const location = useLocation();
  const [showUserMenu, setShowUserMenu] = useState(false);
  const [showMoreMenu, setShowMoreMenu] = useState(false);
  const [showDesktopMoreMenu, setShowDesktopMoreMenu] = useState(false);
  const userMenuRef = useRef<HTMLDivElement>(null);
  const moreMenuRef = useRef<HTMLDivElement>(null);
  const desktopMoreRef = useRef<HTMLDivElement>(null);
  const { data: notifCount } = useQuery({
    queryKey: ['notification-count'],
    queryFn: getNotificationCount,
    staleTime: 25_000,
    refetchInterval: 60_000,
    enabled: isAuthenticated && Boolean(user?.company_id) && !user?.must_change_password,
  });

  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove('theme-admin');
    root.classList.add('theme-portal');
    return () => {
      root.classList.remove('theme-portal');
    };
  }, []);

  // Close dropdowns on outside click or Escape key.
  useEffect(() => {
    const handleMouseDown = (e: MouseEvent) => {
      if (userMenuRef.current && !userMenuRef.current.contains(e.target as Node)) {
        setShowUserMenu(false);
      }
      if (moreMenuRef.current && !moreMenuRef.current.contains(e.target as Node)) {
        setShowMoreMenu(false);
      }
      if (desktopMoreRef.current && !desktopMoreRef.current.contains(e.target as Node)) {
        setShowDesktopMoreMenu(false);
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setShowUserMenu(false);
        setShowMoreMenu(false);
        setShowDesktopMoreMenu(false);
      }
    };
    document.addEventListener('mousedown', handleMouseDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('mousedown', handleMouseDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, []);

  if (isLoading) {
    return (
      <div className="theme-portal flex items-center justify-center min-h-screen bg-[var(--background)]">
        <div className="spinner" />
      </div>
    );
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  if (user?.must_change_password) {
    return <Navigate to="/change-password" replace />;
  }

  const isRouteActive = (href: string) =>
    location.pathname === href || location.pathname.startsWith(href + '/');

  const isMoreActive = mobileMoreNav.some((item) => isRouteActive(item.href));
  const isCompactMoreActive = compactDesktopMoreNav.some((item) => isRouteActive(item.href));
  const noEmployeeProfile = !user?.employee_id;
  const showNoProfileBanner = noEmployeeProfile && !isRouteActive('/portal/notifications');

  const hasUnread = Boolean(notifCount && notifCount.unread > 0);

  return (
    <div className="theme-portal relative isolate min-h-screen bg-[var(--background)]">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50 focus:rounded-md focus:bg-[var(--primary-bg)] focus:px-4 focus:py-2 focus:text-sm focus:font-medium focus:text-[var(--primary-fg)] focus:shadow-md"
      >
        {t('a11y.skipToContent')}
      </a>

      {/* Top Navigation Bar */}
      <header className="safe-area-top sticky top-0 z-50 bg-[var(--card)] border-b border-[var(--border)]">
        <div className="max-w-[1360px] mx-auto flex items-center justify-between gap-2 h-13 px-4 md:px-6">
          {/* Left: Logo + nav */}
          <div className="flex items-center gap-3 lg:gap-5 min-w-0">
            <div className="flex items-center gap-2 shrink-0">
              <BrandLogo variant="lockup-auto" priority className="h-6 sm:h-7 w-auto shrink-0" />
              <span className="hidden lg:inline-flex items-center rounded border border-[var(--border)] bg-[var(--surface-subtle)] px-2 py-0.5 text-[11px] font-medium text-[var(--muted-foreground)]">
                {t('nav.portalBadge')}
              </span>
            </div>

            {/* Full Desktop nav (xl+) */}
            <nav aria-label={t('nav.portalBadge')} className="hidden xl:flex items-center gap-0.5">
              {portalNav.map((item) => {
                const isActive = isRouteActive(item.href);
                return (
                  <Link
                    key={item.nameKey}
                    to={item.href}
                    onMouseEnter={() => prefetchRoute(item.href)}
                    onFocus={() => prefetchRoute(item.href)}
                    aria-current={isActive ? 'page' : undefined}
                    className={`flex items-center gap-1.5 px-2.5 py-1.5 text-[13px] rounded-md transition-all-fast border ${
                      isActive
                        ? 'bg-[var(--surface-subtle)] border-[var(--border)] text-[var(--foreground)] font-medium'
                        : 'border-transparent text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)]/70'
                    }`}
                  >
                    <item.icon className={`w-3.5 h-3.5 shrink-0 ${isActive ? 'text-[var(--accent-1)]' : 'text-[var(--subtle-foreground)]'}`} />
                    <span className="whitespace-nowrap">{t(item.nameKey)}</span>
                  </Link>
                );
              })}
            </nav>

            {/* Compact Desktop nav (md..xl) */}
            <nav aria-label={t('nav.portalBadge')} className="hidden md:flex xl:hidden items-center gap-0.5">
              {compactDesktopPrimaryNav.map((item) => {
                const isActive = isRouteActive(item.href);
                return (
                  <Link
                    key={item.nameKey}
                    to={item.href}
                    onMouseEnter={() => prefetchRoute(item.href)}
                    onFocus={() => prefetchRoute(item.href)}
                    aria-current={isActive ? 'page' : undefined}
                    className={`flex items-center gap-1.5 px-2.5 py-1.5 text-[13px] rounded-md transition-all-fast border ${
                      isActive
                        ? 'bg-[var(--surface-subtle)] border-[var(--border)] text-[var(--foreground)] font-medium'
                        : 'border-transparent text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)]/70'
                    }`}
                  >
                    <item.icon className={`w-3.5 h-3.5 shrink-0 ${isActive ? 'text-[var(--accent-1)]' : 'text-[var(--subtle-foreground)]'}`} />
                    <span className="whitespace-nowrap">{t(item.nameKey)}</span>
                  </Link>
                );
              })}

              <div ref={desktopMoreRef} className="relative">
                <button
                  type="button"
                  onClick={() => setShowDesktopMoreMenu((prev) => !prev)}
                  aria-expanded={showDesktopMoreMenu}
                  aria-haspopup="menu"
                  className={`flex items-center gap-1 px-2.5 py-1.5 text-[13px] rounded-md transition-all-fast border ${
                    isCompactMoreActive
                      ? 'bg-[var(--surface-subtle)] border-[var(--border)] text-[var(--foreground)] font-medium'
                      : 'border-transparent text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)]/70'
                  }`}
                >
                  <MoreHorizontal className="w-4 h-4" />
                  <span className="whitespace-nowrap">{t('nav.portal.more')}</span>
                </button>
                {showDesktopMoreMenu && (
                  <div
                    role="menu"
                    className="animate-drop-in absolute left-0 mt-1.5 w-48 rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-md z-30 py-1 overflow-hidden"
                  >
                    {compactDesktopMoreNav.map((item) => {
                      const active = isRouteActive(item.href);
                      return (
                        <Link
                          key={item.nameKey}
                          to={item.href}
                          role="menuitem"
                          aria-current={active ? 'page' : undefined}
                          onMouseEnter={() => prefetchRoute(item.href)}
                          onClick={() => setShowDesktopMoreMenu(false)}
                          className={`flex items-center justify-between px-3.5 py-2 text-[13px] transition-colors ${
                            active
                              ? 'text-[var(--foreground)] font-medium bg-[var(--surface-subtle)]'
                              : 'text-[var(--muted-foreground)] hover:bg-[var(--surface-subtle)] hover:text-[var(--foreground)]'
                          }`}
                        >
                          <span className="flex items-center gap-2.5">
                            <item.icon className={`w-4 h-4 ${active ? 'text-[var(--accent-1)]' : ''}`} />
                            <span>{t(item.nameKey)}</span>
                          </span>
                        </Link>
                      );
                    })}
                  </div>
                )}
              </div>
            </nav>
          </div>

          {/* Right: Quick search + Persistent notification bell + User dropdown */}
          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={openCommandPalette}
              aria-label={t('nav.quickSearch')}
              className="min-h-11 min-w-11 md:min-h-8 md:min-w-8 md:h-8 md:w-8 inline-flex items-center justify-center text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)] rounded-md transition-all-fast"
            >
              <Search className="w-4 h-4" />
            </button>
            <Link
              to="/portal/notifications"
              aria-label={t('nav.portal.notifications')}
              title={t('nav.portal.notifications')}
              className={`relative min-h-11 min-w-11 md:min-h-8 md:min-w-8 md:h-8 md:w-8 inline-flex items-center justify-center rounded-md transition-all-fast ${
                isRouteActive('/portal/notifications')
                  ? 'bg-[var(--surface-subtle)] text-[var(--foreground)] border border-[var(--border)]'
                  : 'text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)]'
              }`}
            >
              <Bell className="w-5 h-5 md:w-4 md:h-4" />
              {hasUnread && (
                <span className="absolute top-1.5 right-1.5 md:top-0.5 md:right-0.5 min-w-[15px] h-[15px] px-1 text-[9px] font-semibold bg-red-600 text-white rounded-full flex items-center justify-center leading-none tabular-nums">
                  {notifCount!.unread > 99 ? '99+' : notifCount!.unread}
                </span>
              )}
            </Link>

            <div ref={userMenuRef} className="relative">
              <button
                type="button"
                onClick={() => setShowUserMenu(!showUserMenu)}
                aria-expanded={showUserMenu}
                aria-haspopup="menu"
                className="min-h-11 md:min-h-8 flex items-center gap-2 text-[13px] text-[var(--muted-foreground)] hover:text-[var(--foreground)] rounded-md px-1.5 transition-all-fast"
              >
                <div className="w-7 h-7 rounded-md bg-[var(--accent-1)] text-[var(--accent-fg)] flex items-center justify-center text-xs font-semibold shrink-0">
                  {user?.full_name?.[0] || 'U'}
                </div>
                <span className="sr-only lg:not-sr-only font-medium text-[var(--foreground)] max-w-[130px] truncate">{user?.full_name || t('common.user')}</span>
                <ChevronDown
                  className={`hidden lg:block w-3.5 h-3.5 text-[var(--subtle-foreground)] transition-transform duration-150 shrink-0 ${showUserMenu ? 'rotate-180' : ''}`}
                />
              </button>

              {showUserMenu && (
                <div className="animate-drop-in absolute right-0 mt-1.5 w-64 rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-md z-20 py-1 overflow-hidden">
                  <div className="px-3.5 py-2.5 border-b border-[var(--border)]">
                    <p className="text-[13px] font-semibold text-[var(--foreground)] truncate">{user?.full_name}</p>
                    <p className="text-xs text-[var(--muted-foreground)] mt-0.5 truncate">{user?.email}</p>
                  </div>
                  {canUseAdminConsole(user) && (
                    <Link
                      to="/"
                      onClick={() => setShowUserMenu(false)}
                      className="flex items-center gap-2.5 w-full px-3.5 py-2.5 text-[13px] text-[var(--muted-foreground)] border-b border-[var(--border)] hover:bg-[var(--surface-subtle)] hover:text-[var(--foreground)] transition-all-fast"
                    >
                      <LayoutDashboard className="w-4 h-4" /> {t('nav.adminConsole')}
                    </Link>
                  )}
                  <div className="px-3.5 py-2.5 space-y-2 border-b border-[var(--border)]">
                    <AppearanceSwitcher />
                    <LanguageSwitcher />
                  </div>
                  {(canShowInstallGuide ||
                    notificationStatus === 'default' ||
                    notificationStatus === 'ios-install-required') && (
                    <div className="border-b border-[var(--border)] py-1">
                      {canShowInstallGuide && (
                        <button
                          type="button"
                          onClick={() => {
                            setShowUserMenu(false);
                            void triggerInstall();
                          }}
                          className="flex items-center gap-2.5 w-full px-3.5 py-2 text-[13px] text-[var(--muted-foreground)] hover:bg-[var(--surface-subtle)] hover:text-[var(--foreground)] transition-all-fast"
                        >
                          {canPromptInstall ? (
                            <Download className="w-4 h-4" aria-hidden="true" />
                          ) : (
                            <Smartphone className="w-4 h-4" aria-hidden="true" />
                          )}
                          {canPromptInstall
                            ? t('pwa.actions.installApp')
                            : t('pwa.actions.addToHomeScreen')}
                        </button>
                      )}
                      {(notificationStatus === 'default' ||
                        notificationStatus === 'ios-install-required') && (
                        <button
                          type="button"
                          disabled={notificationBusy}
                          onClick={() => {
                            setShowUserMenu(false);
                            void enableNotifications();
                          }}
                          className="flex items-center gap-2.5 w-full px-3.5 py-2 text-[13px] text-[var(--muted-foreground)] hover:bg-[var(--surface-subtle)] hover:text-[var(--foreground)] transition-all-fast disabled:opacity-50"
                        >
                          <Bell className="w-4 h-4" aria-hidden="true" />
                          {t('pwa.actions.allowNotifications')}
                        </button>
                      )}
                    </div>
                  )}
                  <button
                    type="button"
                    onClick={() => void logout()}
                    className="flex items-center gap-2.5 w-full px-3.5 py-2.5 text-[13px] text-[var(--muted-foreground)] hover:bg-[var(--surface-subtle)] hover:text-red-600 dark:hover:text-red-400 transition-all-fast"
                  >
                    <LogOut className="w-4 h-4" /> {t('nav.signOut')}
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>
      </header>

      {/* Content */}
      <main
        id="main-content"
        tabIndex={-1}
        className="max-w-[1360px] mx-auto px-4 py-4 md:px-6 md:py-5 pb-[calc(5rem+env(safe-area-inset-bottom))] md:pb-6 focus:outline-none"
      >
        <PwaInstallBanner />
        {showNoProfileBanner && (
          <div className="mb-4 surface-panel p-3.5 border-amber-300 dark:border-amber-500/30 bg-amber-50/70 dark:bg-amber-500/10 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
            <div className="flex items-start gap-2.5">
              <Info className="w-4 h-4 text-amber-600 dark:text-amber-400 shrink-0 mt-0.5" aria-hidden="true" />
              <div className="text-[13px]">
                <p className="font-medium text-amber-900 dark:text-amber-200">
                  {t('nav.portal.noProfileTitle')}
                </p>
                <p className="text-xs text-amber-800 dark:text-amber-300/90 mt-0.5">
                  {t('nav.portal.noProfileBody')}
                </p>
              </div>
            </div>
            {canUseAdminConsole(user) && (
              <Link to="/" className="btn-secondary !min-h-9 text-xs shrink-0">
                <LayoutDashboard className="w-3.5 h-3.5" aria-hidden="true" />
                {t('nav.portal.backToConsole')}
              </Link>
            )}
          </div>
        )}
        <PageTransition>
          <Outlet />
        </PageTransition>
        <footer className="no-print mt-8 pt-4 border-t border-[var(--border)] flex flex-wrap items-center justify-center gap-3 text-xs text-[var(--subtle-foreground)]">
          <Link to="/terms" className="hover:text-[var(--foreground)] transition-colors">
            {t('legal.links.terms')}
          </Link>
          <span aria-hidden="true">·</span>
          <Link to="/privacy" className="hover:text-[var(--foreground)] transition-colors">
            {t('legal.links.privacy')}
          </Link>
          <span aria-hidden="true">·</span>
          <Link to="/compliance" className="hover:text-[var(--foreground)] transition-colors">
            {t('legal.links.compliance')}
          </Link>
        </footer>
      </main>

      {/* Mobile Bottom Tab Bar */}
      <nav
        aria-label={t('nav.portalBadge')}
        className="md:hidden fixed bottom-0 left-0 right-0 z-50 safe-area-bottom bg-[var(--card)] border-t border-[var(--border)]"
      >
        <div className="flex items-stretch">
          {mobileTabNav.map((item) => {
            const isActive = isRouteActive(item.href);
            return (
              <Link
                key={item.nameKey}
                to={item.href}
                aria-current={isActive ? 'page' : undefined}
                onClick={() => setShowMoreMenu(false)}
                className={`relative z-20 flex-1 min-w-0 flex flex-col items-center gap-0.5 py-2 pt-2.5 text-[10px] font-medium transition-colors ${
                  isActive ? 'text-[var(--foreground)]' : 'text-[var(--subtle-foreground)]'
                }`}
              >
                {isActive && (
                  <span className="absolute top-0 h-[2px] w-8 bg-[var(--accent-1)]" />
                )}
                <item.icon className={`w-5 h-5 ${isActive ? 'text-[var(--accent-1)]' : ''}`} />
                <span className="max-w-full truncate px-0.5">{t(item.nameKey)}</span>
              </Link>
            );
          })}

          {/* More tab */}
          <div ref={moreMenuRef} className="flex-1 min-w-0 relative">
            <button
              type="button"
              onClick={() => setShowMoreMenu(!showMoreMenu)}
              aria-expanded={showMoreMenu}
              aria-haspopup="menu"
              className={`relative w-full flex flex-col items-center gap-0.5 py-2 pt-2.5 text-[10px] font-medium transition-colors ${
                isMoreActive ? 'text-[var(--foreground)]' : 'text-[var(--subtle-foreground)]'
              }`}
            >
              {isMoreActive && (
                <span className="absolute top-0 h-[2px] w-8 bg-[var(--accent-1)]" />
              )}
              <span className="relative">
                <MoreHorizontal className={`w-5 h-5 ${isMoreActive ? 'text-[var(--accent-1)]' : ''}`} />
                {hasUnread && (
                  <span className="absolute -top-0.5 -right-1 h-2 w-2 rounded-full bg-red-600" aria-hidden="true" />
                )}
              </span>
              <span className="max-w-full truncate px-0.5">{t('nav.portal.more')}</span>
            </button>

            {showMoreMenu && (
              <div className="animate-slide-up-sm absolute bottom-full right-2 mb-2 w-48 rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-md z-20 py-1 overflow-hidden">
                {mobileMoreNav.map((item) => {
                  const isActive = isRouteActive(item.href);
                  return (
                    <Link
                      key={item.nameKey}
                      to={item.href}
                      aria-current={isActive ? 'page' : undefined}
                      onClick={() => setShowMoreMenu(false)}
                      className={`flex items-center gap-2.5 px-3.5 py-2.5 text-[13px] transition-colors ${
                        isActive
                          ? 'text-[var(--foreground)] font-medium bg-[var(--surface-subtle)]'
                          : 'text-[var(--muted-foreground)] hover:bg-[var(--surface-subtle)] hover:text-[var(--foreground)]'
                      }`}
                    >
                      <item.icon className={`w-4 h-4 ${isActive ? 'text-[var(--accent-1)]' : ''}`} />
                      {t(item.nameKey)}
                      {item.nameKey === 'nav.portal.notifications' && hasUnread && (
                        <span className="ml-auto min-w-[18px] h-[18px] px-1 text-[10px] font-semibold bg-red-600 text-white rounded-full flex items-center justify-center leading-none tabular-nums">
                          {notifCount!.unread}
                        </span>
                      )}
                    </Link>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </nav>
    </div>
  );
}
