import { useEffect, useRef, useState } from 'react';
import { Link, Outlet, Navigate, useLocation } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import { Bell, Download, Menu, Search, Shield, SlidersHorizontal, Smartphone } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { usePwa } from '@/context/PwaContext';
import { useTranslation } from 'react-i18next';
import { getNotificationCount } from '@/api/notifications';
import { Sidebar } from './Sidebar';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { PageTransition } from '@/components/ui/PageTransition';
import { PwaInstallBanner } from '@/components/pwa/PwaInstallBanner';
import { AppearanceSwitcher } from '@/components/ui/AppearanceSwitcher';
import { LanguageSwitcher } from '@/components/ui/LanguageSwitcher';
import { openCommandPalette } from '@/lib/commandPalette';
import { canUseAdminConsole } from '@/lib/roles';
import { ROUTES } from '@/lib/routes';

export function AppLayout() {
  const { user, isAuthenticated, isLoading } = useAuth();
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
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [prefsOpen, setPrefsOpen] = useState(false);
  const prefsRef = useRef<HTMLDivElement>(null);

  const { data: notifCount } = useQuery({
    queryKey: ['notification-count'],
    queryFn: getNotificationCount,
    refetchInterval: 30_000,
    enabled: isAuthenticated && Boolean(user?.company_id) && !user?.must_change_password,
  });
  const hasUnread = Boolean(notifCount && notifCount.unread > 0);
  const hasPwaAction =
    canShowInstallGuide ||
    notificationStatus === 'default' ||
    notificationStatus === 'ios-install-required';

  // Sync shell accent class to <html> so portaled modals, ConfirmDialog, and
  // CommandPalette inherit --accent-1/--accent-2/--ring in both light and dark modes.
  useEffect(() => {
    const root = document.documentElement;
    root.classList.add('theme-admin');
    root.classList.remove('theme-portal');
    return () => {
      root.classList.remove('theme-admin');
    };
  }, []);

  useEffect(() => {
    const handleMouseDown = (e: MouseEvent) => {
      if (prefsRef.current && !prefsRef.current.contains(e.target as Node)) {
        setPrefsOpen(false);
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setPrefsOpen(false);
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
      <div className="theme-admin flex items-center justify-center min-h-screen bg-[var(--background)]">
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

  if (user && !canUseAdminConsole(user)) {
    return <Navigate to="/portal" replace />;
  }

  const onNotificationsRoute = location.pathname === ROUTES.NOTIFICATIONS;

  return (
    <div className="theme-admin relative isolate flex h-[100dvh] overflow-hidden bg-[var(--background)]">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50 focus:rounded-lg focus:bg-indigo-600 focus:px-4 focus:py-2 focus:text-sm focus:font-medium focus:text-white focus:shadow-lg"
      >
        {t('a11y.skipToContent')}
      </a>
      {/* Ambient aurora behind the whole console */}
      <div aria-hidden className="aurora-admin pointer-events-none absolute inset-0 -z-10" />

      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />

      <div className="flex-1 flex flex-col min-w-0">
        {/* Unified responsive top utility bar */}
        <header className="glass-nav safe-area-top sticky top-0 z-30 shrink-0">
          <div className="flex items-center gap-2 sm:gap-3 h-14 px-4 md:px-8">
            <button
              type="button"
              onClick={() => setSidebarOpen(true)}
              aria-label={t('a11y.openMenu')}
              className="md:hidden min-h-11 min-w-11 inline-flex items-center justify-center -ml-2 text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800 rounded-lg transition-all-fast shrink-0"
            >
              <Menu className="w-5 h-5" />
            </button>
            <BrandLogo variant="lockup-dark" priority className="md:hidden h-7 w-auto shrink-0 dark:brightness-125" />
            <span className="hidden md:inline-flex items-center gap-1.5 rounded-full border border-indigo-200/60 dark:border-indigo-400/30 bg-indigo-50/80 dark:bg-indigo-500/15 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-widest text-indigo-600 dark:text-indigo-300">
              <span className="glow-dot" />
              {t('nav.adminConsoleBadge')}
            </span>

            <div className="ml-auto flex items-center gap-1 sm:gap-1.5">
              <button
                type="button"
                onClick={openCommandPalette}
                aria-label={t('nav.quickSearch')}
                className="min-h-11 min-w-11 md:min-h-9 md:w-56 lg:w-64 inline-flex items-center justify-center md:justify-between md:px-3 rounded-xl text-xs text-gray-500 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800 md:border md:border-gray-200/80 dark:md:border-slate-700/80 md:bg-white/75 dark:md:bg-slate-900/70 md:hover:border-indigo-400/50 transition-all-fast"
              >
                <span className="inline-flex items-center gap-2 min-w-0">
                  <Search className="w-4 h-4 md:w-3.5 md:h-3.5 shrink-0" aria-hidden="true" />
                  <span className="hidden md:inline truncate">{t('nav.quickSearch')}</span>
                </span>
                <kbd className="hidden md:inline-flex rounded border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 px-1.5 py-0.5 text-[10px] font-semibold text-gray-400 shrink-0">
                  ⌘K
                </kbd>
              </button>

              {Boolean(user?.company_id) && (
                <Link
                  to={ROUTES.NOTIFICATIONS}
                  aria-label={t('nav.portal.notifications')}
                  title={t('nav.portal.notifications')}
                  className={`relative min-h-11 min-w-11 md:min-h-9 md:min-w-9 inline-flex items-center justify-center rounded-xl transition-all-fast ${
                    onNotificationsRoute
                      ? 'bg-indigo-50 dark:bg-indigo-500/20 text-indigo-600 dark:text-indigo-300 ring-1 ring-indigo-500/25'
                      : 'text-gray-500 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800'
                  }`}
                >
                  <Bell className="w-4 h-4" aria-hidden="true" />
                  {hasUnread && (
                    <span className="absolute top-1.5 right-1.5 md:top-0.5 md:right-0.5 min-w-[16px] h-[16px] px-1 text-[9px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none shadow-[0_0_8px_rgba(239,68,68,0.5)]">
                      {notifCount!.unread > 99 ? '99+' : notifCount!.unread}
                    </span>
                  )}
                </Link>
              )}

              <div ref={prefsRef} className="relative">
                <button
                  type="button"
                  onClick={() => setPrefsOpen((prev) => !prev)}
                  aria-expanded={prefsOpen}
                  aria-haspopup="true"
                  aria-label={t('nav.preferences')}
                  title={t('nav.preferences')}
                  className={`relative min-h-11 min-w-11 md:min-h-9 md:min-w-9 inline-flex items-center justify-center rounded-xl transition-all-fast ${
                    prefsOpen
                      ? 'bg-indigo-50 dark:bg-indigo-500/20 text-indigo-600 dark:text-indigo-300 ring-1 ring-indigo-500/25'
                      : 'text-gray-500 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800'
                  }`}
                >
                  <SlidersHorizontal className="w-4 h-4" aria-hidden="true" />
                  {hasPwaAction && (
                    <span
                      className="absolute top-2 right-2 md:top-1.5 md:right-1.5 h-2 w-2 rounded-full bg-indigo-500"
                      aria-hidden="true"
                    />
                  )}
                </button>

                {prefsOpen && (
                  <div className="animate-drop-in glass-menu absolute right-0 mt-2 w-64 rounded-2xl z-40 py-1 overflow-hidden">
                    <div className="px-4 py-3 space-y-2.5 border-b border-gray-200/60 dark:border-slate-800">
                      <AppearanceSwitcher />
                      <LanguageSwitcher />
                    </div>
                    {hasPwaAction && (
                      <div className="border-b border-gray-200/60 dark:border-slate-800 py-1">
                        {canShowInstallGuide && (
                          <button
                            type="button"
                            onClick={() => {
                              setPrefsOpen(false);
                              void triggerInstall();
                            }}
                            className="flex items-center gap-2.5 w-full px-4 py-2.5 text-sm text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-800 hover:text-indigo-600 dark:hover:text-indigo-300 transition-all-fast"
                          >
                            {canPromptInstall ? (
                              <Download className="w-4 h-4 shrink-0" aria-hidden="true" />
                            ) : (
                              <Smartphone className="w-4 h-4 shrink-0" aria-hidden="true" />
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
                              setPrefsOpen(false);
                              void enableNotifications();
                            }}
                            className="flex items-center gap-2.5 w-full px-4 py-2.5 text-sm text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-800 hover:text-indigo-600 dark:hover:text-indigo-300 transition-all-fast disabled:opacity-50"
                          >
                            <Bell className="w-4 h-4 shrink-0" aria-hidden="true" />
                            <span className="truncate">{t('pwa.actions.allowNotifications')}</span>
                          </button>
                        )}
                      </div>
                    )}
                    <Link
                      to={ROUTES.ACCOUNT}
                      onClick={() => setPrefsOpen(false)}
                      className="flex items-center gap-2.5 w-full px-4 py-3 text-sm text-gray-600 dark:text-slate-300 hover:bg-white/70 dark:hover:bg-slate-800 hover:text-indigo-600 dark:hover:text-indigo-300 transition-all-fast"
                    >
                      <Shield className="w-4 h-4 shrink-0" aria-hidden="true" />
                      <span>{t('nav.accountSecurity')}</span>
                    </Link>
                  </div>
                )}
              </div>
            </div>
          </div>
        </header>

        <main id="main-content" tabIndex={-1} className="flex-1 overflow-auto focus:outline-none">
          <div className="max-w-7xl mx-auto px-4 py-4 pb-[max(1rem,env(safe-area-inset-bottom))] md:px-8 md:py-6">
            <PwaInstallBanner />
            <PageTransition>
              <Outlet />
            </PageTransition>
            <footer className="no-print mt-10 pt-4 border-t border-slate-200/60 dark:border-slate-800/70 flex flex-wrap items-center justify-center gap-3 text-xs text-slate-500 dark:text-slate-400">
              <Link to={ROUTES.TERMS} className="hover:text-indigo-600 dark:hover:text-indigo-300 transition-colors">
                {t('legal.links.terms')}
              </Link>
              <span>·</span>
              <Link to={ROUTES.PRIVACY} className="hover:text-indigo-600 dark:hover:text-indigo-300 transition-colors">
                {t('legal.links.privacy')}
              </Link>
              <span>·</span>
              <Link to={ROUTES.COMPLIANCE} className="hover:text-indigo-600 dark:hover:text-indigo-300 transition-colors">
                {t('legal.links.compliance')}
              </Link>
            </footer>
          </div>
        </main>
      </div>
    </div>
  );
}
