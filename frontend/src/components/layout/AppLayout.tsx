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
    staleTime: 25_000,
    refetchInterval: 60_000,
    enabled: isAuthenticated && Boolean(user?.company_id) && !user?.must_change_password,
  });
  const hasUnread = Boolean(notifCount && notifCount.unread > 0);
  const hasPwaAction =
    canShowInstallGuide ||
    notificationStatus === 'default' ||
    notificationStatus === 'ios-install-required';

  // Sync shell accent class to <html> so portaled modals, ConfirmDialog, and
  // CommandPalette inherit --accent-1/--ring in both light and dark modes.
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
        className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50 focus:rounded-md focus:bg-[var(--primary-bg)] focus:px-4 focus:py-2 focus:text-sm focus:font-medium focus:text-[var(--primary-fg)] focus:shadow-md"
      >
        {t('a11y.skipToContent')}
      </a>

      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />

      <div className="flex-1 flex flex-col min-w-0">
        {/* Clean flat top bar */}
        <header className="safe-area-top sticky top-0 z-30 shrink-0 bg-[var(--card)] border-b border-[var(--border)]">
          <div className="flex items-center gap-2 sm:gap-3 h-12 px-4 md:px-6">
            <button
              type="button"
              onClick={() => setSidebarOpen(true)}
              aria-label={t('a11y.openMenu')}
              className="md:hidden min-h-11 min-w-11 inline-flex items-center justify-center -ml-2 text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)] rounded-md transition-all-fast shrink-0"
            >
              <Menu className="w-5 h-5" />
            </button>
            <BrandLogo variant="lockup-auto" priority className="md:hidden h-6 w-auto shrink-0" />
            <span className="hidden md:inline-flex items-center rounded border border-[var(--border)] bg-[var(--surface-subtle)] px-2 py-0.5 text-[11px] font-medium text-[var(--muted-foreground)]">
              {t('nav.adminConsoleBadge')}
            </span>

            <div className="ml-auto flex items-center gap-1 sm:gap-1.5">
              <button
                type="button"
                onClick={openCommandPalette}
                aria-label={t('nav.quickSearch')}
                className="min-h-11 min-w-11 md:min-h-8 md:h-8 md:w-52 lg:w-60 inline-flex items-center justify-center md:justify-between md:px-2.5 rounded-md text-xs text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)] md:border md:border-[var(--border)] md:bg-[var(--surface-subtle)] md:hover:border-[var(--border-strong)] transition-all-fast"
              >
                <span className="inline-flex items-center gap-2 min-w-0">
                  <Search className="w-4 h-4 md:w-3.5 md:h-3.5 shrink-0" aria-hidden="true" />
                  <span className="hidden md:inline truncate">{t('nav.quickSearch')}</span>
                </span>
                <kbd className="hidden md:inline-flex rounded border border-[var(--border)] bg-[var(--card)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--subtle-foreground)] shrink-0">
                  ⌘K
                </kbd>
              </button>

              {Boolean(user?.company_id) && (
                <Link
                  to={ROUTES.NOTIFICATIONS}
                  aria-label={t('nav.portal.notifications')}
                  title={t('nav.portal.notifications')}
                  className={`relative min-h-11 min-w-11 md:min-h-8 md:min-w-8 md:h-8 md:w-8 inline-flex items-center justify-center rounded-md transition-all-fast ${
                    onNotificationsRoute
                      ? 'bg-[var(--surface-subtle)] text-[var(--foreground)] border border-[var(--border)]'
                      : 'text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)]'
                  }`}
                >
                  <Bell className="w-4 h-4" aria-hidden="true" />
                  {hasUnread && (
                    <span className="absolute top-1.5 right-1.5 md:top-0.5 md:right-0.5 min-w-[15px] h-[15px] px-1 text-[9px] font-semibold bg-red-600 text-white rounded-full flex items-center justify-center leading-none tabular-nums">
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
                  className={`relative min-h-11 min-w-11 md:min-h-8 md:min-w-8 md:h-8 md:w-8 inline-flex items-center justify-center rounded-md transition-all-fast ${
                    prefsOpen
                      ? 'bg-[var(--surface-subtle)] text-[var(--foreground)] border border-[var(--border)]'
                      : 'text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--surface-subtle)]'
                  }`}
                >
                  <SlidersHorizontal className="w-4 h-4" aria-hidden="true" />
                  {hasPwaAction && (
                    <span
                      className="absolute top-2 right-2 md:top-1 md:right-1 h-1.5 w-1.5 rounded-full bg-[var(--accent-1)]"
                      aria-hidden="true"
                    />
                  )}
                </button>

                {prefsOpen && (
                  <div className="animate-drop-in absolute right-0 mt-1.5 w-64 rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-md z-40 py-1 overflow-hidden">
                    <div className="flex items-center gap-2 px-3.5 py-2.5 border-b border-[var(--border)]">
                      <div className="min-w-0 flex-1">
                        <AppearanceSwitcher />
                      </div>
                      <LanguageSwitcher />
                    </div>
                    {hasPwaAction && (
                      <div className="border-b border-[var(--border)] py-1">
                        {canShowInstallGuide && (
                          <button
                            type="button"
                            onClick={() => {
                              setPrefsOpen(false);
                              void triggerInstall();
                            }}
                            className="flex items-center gap-2.5 w-full px-3.5 py-2 text-[13px] text-[var(--muted-foreground)] hover:bg-[var(--surface-subtle)] hover:text-[var(--foreground)] transition-all-fast"
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
                            className="flex items-center gap-2.5 w-full px-3.5 py-2 text-[13px] text-[var(--muted-foreground)] hover:bg-[var(--surface-subtle)] hover:text-[var(--foreground)] transition-all-fast disabled:opacity-50"
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
                      className="flex items-center gap-2.5 w-full px-3.5 py-2.5 text-[13px] text-[var(--muted-foreground)] hover:bg-[var(--surface-subtle)] hover:text-[var(--foreground)] transition-all-fast"
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
          <div className="max-w-[1360px] mx-auto px-4 py-4 pb-[max(1rem,env(safe-area-inset-bottom))] md:px-6 md:py-5">
            <PwaInstallBanner />
            <PageTransition>
              <Outlet />
            </PageTransition>
            <footer className="no-print mt-8 pt-4 border-t border-[var(--border)] flex flex-wrap items-center justify-center gap-3 text-xs text-[var(--subtle-foreground)]">
              <Link to={ROUTES.TERMS} className="hover:text-[var(--foreground)] transition-colors">
                {t('legal.links.terms')}
              </Link>
              <span aria-hidden="true">·</span>
              <Link to={ROUTES.PRIVACY} className="hover:text-[var(--foreground)] transition-colors">
                {t('legal.links.privacy')}
              </Link>
              <span aria-hidden="true">·</span>
              <Link to={ROUTES.COMPLIANCE} className="hover:text-[var(--foreground)] transition-colors">
                {t('legal.links.compliance')}
              </Link>
            </footer>
          </div>
        </main>
      </div>
    </div>
  );
}
