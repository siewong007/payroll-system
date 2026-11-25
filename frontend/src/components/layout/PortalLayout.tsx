import { Outlet, Link, useLocation, Navigate } from 'react-router';
import { User, FileText, Calendar, Receipt, LogOut, ChevronDown, Bell, Users, Clock, MoreHorizontal, ScanLine, Sparkles, LayoutDashboard, Search, Download, Smartphone, Info } from 'lucide-react';
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
  { nameKey: 'nav.portal.notifications', href: '/portal/notifications', icon: Bell },
];

// Compact desktop nav for md..lg viewports before xl fits all 8 links
const compactDesktopPrimaryNav = portalNav.slice(0, 5);
const compactDesktopMoreNav = portalNav.slice(5);

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
    refetchInterval: 30_000,
    enabled: isAuthenticated && !user?.must_change_password,
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

  const hasUnread = notifCount && notifCount.unread > 0;

  return (
    <div className="theme-portal relative isolate min-h-screen bg-[var(--background)]">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50 focus:rounded-lg focus:bg-teal-600 focus:px-4 focus:py-2 focus:text-sm focus:font-medium focus:text-white focus:shadow-lg"
      >
        {t('a11y.skipToContent')}
      </a>
      {/* Ambient aurora behind the portal */}
      <div aria-hidden className="aurora-portal pointer-events-none fixed inset-0 -z-10" />

      {/* Top Navigation Bar */}
      <header className="glass-nav safe-area-top sticky top-0 z-50">
        <div className="max-w-7xl mx-auto flex items-center justify-between h-14 md:h-16 px-4 md:px-6">
          {/* Left: Logo + nav */}
          <div className="flex items-center gap-4 lg:gap-6 xl:gap-8">
            <div className="flex items-center gap-2">
              <BrandLogo variant="lockup-dark" priority className="h-7 sm:h-8 w-auto shrink-0" />
              <span className="hidden sm:inline-flex items-center gap-1 bg-gradient-to-r from-teal-500 to-emerald-500 text-white text-[10px] font-semibold px-2 py-0.5 rounded-full shadow-[0_2px_10px_-2px_var(--glow)]">
                <Sparkles className="w-2.5 h-2.5" />
                {t('nav.portalBadge')}
              </span>
            </div>

            {/* Full Desktop nav (xl+) */}
            <nav aria-label={t('nav.portalBadge')} className="hidden xl:flex items-center gap-1">
              {portalNav.map((item) => {
                const isActive = isRouteActive(item.href);
                return (
                  <Link
                    key={item.nameKey}
                    to={item.href}
                    aria-current={isActive ? 'page' : undefined}
                    className={`relative flex items-center gap-1.5 px-3 py-2 text-sm rounded-xl transition-all-fast ${
                      isActive
                        ? 'text-teal-900 dark:text-teal-200 font-semibold'
                        : 'text-gray-500 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800/70'
                    }`}
                  >
                    {isActive && (
                      <span className="absolute inset-0 rounded-xl bg-gradient-to-r from-teal-500/15 to-emerald-500/15 ring-1 ring-teal-500/25 shadow-[0_4px_14px_-6px_var(--glow)]" />
                    )}
                    <item.icon className={`relative w-4 h-4 ${isActive ? 'text-teal-600 dark:text-teal-300' : ''}`} />
                    <span className="relative whitespace-nowrap">{t(item.nameKey)}</span>
                    {item.nameKey === 'nav.portal.notifications' && hasUnread && (
                      <span className="absolute -top-0.5 -right-0.5 min-w-[18px] h-[18px] px-1 text-[10px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none shadow-[0_0_10px_rgba(239,68,68,0.6)]">
                        {notifCount.unread}
                      </span>
                    )}
                  </Link>
                );
              })}
            </nav>

            {/* Compact Desktop nav (md..xl) */}
            <nav aria-label={t('nav.portalBadge')} className="hidden md:flex xl:hidden items-center gap-1">
              {compactDesktopPrimaryNav.map((item) => {
                const isActive = isRouteActive(item.href);
                return (
                  <Link
                    key={item.nameKey}
                    to={item.href}
                    aria-current={isActive ? 'page' : undefined}
                    className={`relative flex items-center gap-1.5 px-2.5 py-2 text-sm rounded-xl transition-all-fast ${
                      isActive
                        ? 'text-teal-900 dark:text-teal-200 font-semibold'
                        : 'text-gray-500 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800/70'
                    }`}
                  >
                    {isActive && (
                      <span className="absolute inset-0 rounded-xl bg-gradient-to-r from-teal-500/15 to-emerald-500/15 ring-1 ring-teal-500/25 shadow-[0_4px_14px_-6px_var(--glow)]" />
                    )}
                    <item.icon className={`relative w-4 h-4 ${isActive ? 'text-teal-600 dark:text-teal-300' : ''}`} />
                    <span className="relative whitespace-nowrap">{t(item.nameKey)}</span>
                  </Link>
                );
              })}

              <div ref={desktopMoreRef} className="relative">
                <button
                  type="button"
                  onClick={() => setShowDesktopMoreMenu((prev) => !prev)}
                  aria-expanded={showDesktopMoreMenu}
                  aria-haspopup="menu"
                  className={`relative flex items-center gap-1 px-2.5 py-2 text-sm rounded-xl transition-all-fast ${
                    isCompactMoreActive
                      ? 'text-teal-900 dark:text-teal-200 font-semibold'
                      : 'text-gray-500 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800/70'
                  }`}
                >
                  {isCompactMoreActive && (
                    <span className="absolute inset-0 rounded-xl bg-gradient-to-r from-teal-500/15 to-emerald-500/15 ring-1 ring-teal-500/25 shadow-[0_4px_14px_-6px_var(--glow)]" />
                  )}
                  <MoreHorizontal className="relative w-4 h-4" />
                  <span className="relative whitespace-nowrap">{t('nav.portal.more')}</span>
                  {hasUnread && (
                    <span className="relative min-w-[16px] h-[16px] px-1 text-[9px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none">
                      {notifCount.unread}
                    </span>
                  )}
                </button>
                {showDesktopMoreMenu && (
                  <div
                    role="menu"
                    className="animate-drop-in glass-menu absolute left-0 mt-2 w-52 rounded-2xl z-30 py-1 overflow-hidden"
                  >
                    {compactDesktopMoreNav.map((item) => {
                      const active = isRouteActive(item.href);
                      return (
                        <Link
                          key={item.nameKey}
                          to={item.href}
                          role="menuitem"
                          aria-current={active ? 'page' : undefined}
                          onClick={() => setShowDesktopMoreMenu(false)}
                          className={`flex items-center justify-between px-4 py-2.5 text-sm transition-colors ${
                            active
                              ? 'text-teal-700 dark:text-teal-300 font-semibold bg-teal-50/70 dark:bg-teal-500/15'
                              : 'text-gray-700 dark:text-slate-200 hover:bg-white/70 dark:hover:bg-slate-800'
                          }`}
                        >
                          <span className="flex items-center gap-2.5">
                            <item.icon className="w-4 h-4" />
                            <span>{t(item.nameKey)}</span>
                          </span>
                          {item.nameKey === 'nav.portal.notifications' && hasUnread && (
                            <span className="min-w-[18px] h-[18px] px-1 text-[10px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none">
                              {notifCount.unread}
                            </span>
                          )}
                        </Link>
                      );
                    })}
                  </div>
                )}
              </div>
            </nav>
          </div>

          {/* Right: Quick search + Mobile notification bell + User dropdown */}
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={openCommandPalette}
              aria-label={t('nav.quickSearch')}
              className="min-h-11 min-w-11 inline-flex items-center justify-center text-gray-500 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800 rounded-lg transition-all-fast"
            >
              <Search className="w-4 h-4" />
            </button>
            {/* Mobile notification bell */}
            <Link
              to="/portal/notifications"
              aria-label={t('nav.portal.notifications')}
              className="relative md:hidden min-h-11 min-w-11 inline-flex items-center justify-center text-gray-500 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800 rounded-lg transition-all-fast"
            >
              <Bell className="w-5 h-5" />
              {hasUnread && (
                <span className="absolute top-1 right-1 min-w-[16px] h-[16px] px-0.5 text-[9px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none shadow-[0_0_8px_rgba(239,68,68,0.6)]">
                  {notifCount.unread}
                </span>
              )}
            </Link>

            <div ref={userMenuRef} className="relative">
              <button
                type="button"
                onClick={() => setShowUserMenu(!showUserMenu)}
                aria-expanded={showUserMenu}
                aria-haspopup="menu"
                className="min-h-11 flex items-center gap-2.5 text-sm text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white rounded-xl px-1.5 transition-all-fast"
              >
                <div className="w-8 h-8 rounded-full bg-gradient-to-br from-teal-500 to-emerald-500 ring-2 ring-white/80 dark:ring-slate-800 shadow-[0_4px_14px_-4px_var(--glow)] flex items-center justify-center text-xs font-bold text-white">
                  {user?.full_name?.[0] || 'U'}
                </div>
                <span className="sr-only md:not-sr-only font-medium">{user?.full_name || t('common.user')}</span>
                <ChevronDown
                  className={`hidden md:block w-3.5 h-3.5 text-gray-400 dark:text-slate-400 transition-transform duration-200 ${showUserMenu ? 'rotate-180' : ''}`}
                />
              </button>

              {showUserMenu && (
                <div className="animate-drop-in glass-menu absolute right-0 mt-2 w-52 rounded-2xl z-20 py-1 overflow-hidden">
                    <div className="px-4 py-3 border-b border-gray-200/60 dark:border-slate-800">
                      <p className="text-sm font-semibold text-gray-900 dark:text-slate-100">{user?.full_name}</p>
                      <p className="text-xs text-gray-500 dark:text-slate-400 mt-0.5">{user?.email}</p>
                    </div>
                    {/* Anyone who is not a sole-role employee lives in the admin
                        shell — `AppLayout` only redirects the sole-role case — so
                        they are here by choice and need a way back. Until now that
                        was the browser's back button or a hand-typed URL.
                        `/` rather than a fixed page: `HomeRedirect` picks the
                        landing screen their roles actually allow, which is
                        `/companies` for a super_admin and `/company` otherwise. */}
                    {canUseAdminConsole(user) && (
                      <Link
                        to="/"
                        onClick={() => setShowUserMenu(false)}
                        className="flex items-center gap-2.5 w-full px-4 py-3 text-sm text-gray-600 dark:text-slate-300 border-b border-gray-200/60 dark:border-slate-800 hover:bg-white/70 dark:hover:bg-slate-800 hover:text-teal-700 dark:hover:text-teal-300 transition-all-fast"
                      >
                        <LayoutDashboard className="w-4 h-4" /> {t('nav.adminConsole')}
                      </Link>
                    )}
                    <div className="px-4 py-2.5 space-y-2 border-b border-gray-200/60 dark:border-slate-800">
                      <AppearanceSwitcher />
                      <LanguageSwitcher />
                    </div>
                    {(canShowInstallGuide ||
                      notificationStatus === 'default' ||
                      notificationStatus === 'ios-install-required') && (
                      <div className="border-b border-gray-200/60 dark:border-slate-800">
                        {canShowInstallGuide && (
                          <button
                            type="button"
                            onClick={() => {
                              setShowUserMenu(false);
                              void triggerInstall();
                            }}
                            className="flex items-center gap-2.5 w-full px-4 py-2.5 text-sm text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-800 hover:text-teal-700 dark:hover:text-teal-300 transition-all-fast"
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
                            className="flex items-center gap-2.5 w-full px-4 py-2.5 text-sm text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-800 hover:text-teal-700 dark:hover:text-teal-300 transition-all-fast disabled:opacity-50"
                          >
                            <Bell className="w-4 h-4" aria-hidden="true" />
                            {t('pwa.actions.allowNotifications')}
                          </button>
                        )}
                      </div>
                    )}
                    <button
                      type="button"
                      onClick={logout}
                      className="flex items-center gap-2.5 w-full px-4 py-3 text-sm text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-800 hover:text-red-600 dark:hover:text-red-400 transition-all-fast"
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
      {/* The bottom bar is ~56px plus env(safe-area-inset-bottom) (~34px on a
          phone with a home indicator), so a flat pb-20 leaves the last row of
          content under it on exactly the devices employees check in from. */}
      <main
        id="main-content"
        tabIndex={-1}
        className="max-w-7xl mx-auto px-4 py-4 md:px-6 md:py-8 pb-[calc(5rem+env(safe-area-inset-bottom))] md:pb-8 focus:outline-none"
      >
        <PwaInstallBanner />
        {noEmployeeProfile && (
          <div className="mb-4 card !p-4 border-amber-200/80 dark:border-amber-500/30 bg-amber-50/80 dark:bg-amber-500/10 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
            <div className="flex items-start gap-3">
              <Info className="w-5 h-5 text-amber-600 dark:text-amber-400 shrink-0 mt-0.5" aria-hidden="true" />
              <div className="text-sm">
                <p className="font-semibold text-amber-900 dark:text-amber-200">
                  {t('nav.portal.noProfileTitle')}
                </p>
                <p className="text-xs text-amber-800/90 dark:text-amber-300/90 mt-0.5">
                  {t('nav.portal.noProfileBody')}
                </p>
              </div>
            </div>
            {canUseAdminConsole(user) && (
              <Link to="/" className="btn-secondary !min-h-10 text-xs shrink-0">
                <LayoutDashboard className="w-3.5 h-3.5" aria-hidden="true" />
                {t('nav.portal.backToConsole')}
              </Link>
            )}
          </div>
        )}
        <PageTransition>
          <Outlet />
        </PageTransition>
        <footer className="no-print mt-10 pt-4 border-t border-slate-200/60 dark:border-slate-800/70 flex flex-wrap items-center justify-center gap-3 text-xs text-slate-500 dark:text-slate-400">
          <Link to="/terms" className="hover:text-teal-600 dark:hover:text-teal-300 transition-colors">
            {t('legal.links.terms')}
          </Link>
          <span>·</span>
          <Link to="/privacy" className="hover:text-teal-600 dark:hover:text-teal-300 transition-colors">
            {t('legal.links.privacy')}
          </Link>
          <span>·</span>
          <Link to="/compliance" className="hover:text-teal-600 dark:hover:text-teal-300 transition-colors">
            {t('legal.links.compliance')}
          </Link>
        </footer>
      </main>

      {/* Mobile Bottom Tab Bar */}
      <nav
        aria-label={t('nav.portalBadge')}
        className="md:hidden fixed bottom-0 left-0 right-0 z-50 safe-area-bottom bg-white/80 dark:bg-slate-900/85 backdrop-blur-xl border-t border-white/60 dark:border-slate-800 shadow-[0_-4px_24px_-12px_var(--glow)]"
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
                  isActive ? 'text-teal-600 dark:text-teal-300' : 'text-gray-400 dark:text-slate-400'
                }`}
              >
                {isActive && (
                  <span className="absolute top-0 h-[3px] w-8 rounded-b-full bg-gradient-to-r from-teal-500 to-emerald-500 shadow-[0_2px_8px_var(--glow)]" />
                )}
                <item.icon className="w-5 h-5" />
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
                isMoreActive ? 'text-teal-600 dark:text-teal-300' : 'text-gray-400 dark:text-slate-400'
              }`}
            >
              {isMoreActive && (
                <span className="absolute top-0 h-[3px] w-8 rounded-b-full bg-gradient-to-r from-teal-500 to-emerald-500 shadow-[0_2px_8px_var(--glow)]" />
              )}
              <span className="relative">
                <MoreHorizontal className="w-5 h-5" />
                {hasUnread && (
                  <span className="absolute -top-0.5 -right-1 h-2 w-2 rounded-full bg-red-500" aria-hidden="true" />
                )}
              </span>
              <span className="max-w-full truncate px-0.5">{t('nav.portal.more')}</span>
            </button>

            {showMoreMenu && (
              <div className="animate-slide-up-sm glass-menu absolute bottom-full right-2 mb-2 w-48 rounded-2xl z-20 py-1 overflow-hidden">
                    {mobileMoreNav.map((item) => {
                      const isActive = isRouteActive(item.href);
                      return (
                        <Link
                          key={item.nameKey}
                          to={item.href}
                          aria-current={isActive ? 'page' : undefined}
                          onClick={() => setShowMoreMenu(false)}
                          className={`flex items-center gap-3 px-4 py-3 text-sm transition-colors ${
                            isActive
                              ? 'text-teal-800 dark:text-teal-200 font-semibold bg-teal-500/10'
                              : 'text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-800'
                          }`}
                        >
                          <item.icon className={`w-4 h-4 ${isActive ? 'text-teal-600 dark:text-teal-300' : ''}`} />
                          {t(item.nameKey)}
                          {item.nameKey === 'nav.portal.notifications' && hasUnread && (
                            <span className="ml-auto min-w-[18px] h-[18px] px-1 text-[10px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none">
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
