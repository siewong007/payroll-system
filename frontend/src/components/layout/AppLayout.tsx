import { useState } from 'react';
import { Outlet, Navigate } from 'react-router';
import { Menu, Search } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { useTranslation } from 'react-i18next';
import { Sidebar } from './Sidebar';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { PageTransition } from '@/components/ui/PageTransition';
import { PwaInstallBanner } from '@/components/pwa/PwaInstallBanner';
import { openCommandPalette } from '@/lib/commandPalette';
import { hasOnlyEmployeeRole } from '@/lib/roles';

export function AppLayout() {
  const { user, isAuthenticated, isLoading } = useAuth();
  const { t } = useTranslation();
  const [sidebarOpen, setSidebarOpen] = useState(false);

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

  if (hasOnlyEmployeeRole(user)) {
    return <Navigate to="/portal" replace />;
  }

  return (
    <div className="theme-admin relative isolate flex h-screen overflow-hidden bg-[var(--background)]">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50 focus:rounded-lg focus:bg-indigo-600 focus:px-4 focus:py-2 focus:text-sm focus:font-medium focus:text-white focus:shadow-lg"
      >
        {t('a11y.skipToContent')}
      </a>
      {/* Ambient aurora behind the whole console */}
      <div aria-hidden className="pointer-events-none absolute inset-0 -z-10 overflow-hidden">
        <div className="ambient-blob animate-float-a -top-32 left-1/4 h-96 w-96 bg-indigo-300/40" />
        <div className="ambient-blob animate-float-b top-1/3 -right-40 h-[28rem] w-[28rem] bg-violet-300/30" />
        <div className="ambient-blob animate-float-a -bottom-32 left-1/2 h-80 w-80 bg-sky-300/30" />
      </div>

      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />

      <div className="flex-1 flex flex-col min-w-0">
        {/* Mobile top bar */}
        <div className="md:hidden glass-nav safe-area-top sticky top-0 z-30 flex items-center gap-3 px-4 py-3">
          <button
            onClick={() => setSidebarOpen(true)}
            aria-label={t('a11y.openMenu')}
            className="min-h-11 min-w-11 inline-flex items-center justify-center -ml-2 text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800 rounded-lg transition-all-fast"
          >
            <Menu className="w-5 h-5" />
          </button>
          <BrandLogo variant="lockup-dark" className="h-7 w-auto dark:brightness-125" />
          <div className="ml-auto flex items-center gap-2">
            <button
              type="button"
              onClick={openCommandPalette}
              aria-label={t('nav.quickSearch')}
              className="min-h-11 min-w-11 inline-flex items-center justify-center text-gray-500 dark:text-slate-300 hover:text-gray-900 dark:hover:text-white hover:bg-white/60 dark:hover:bg-slate-800 rounded-lg transition-all-fast"
            >
              <Search className="w-4 h-4" />
            </button>
            <span className="inline-flex items-center gap-1.5 rounded-full border border-indigo-200/60 dark:border-indigo-400/30 bg-indigo-50/80 dark:bg-indigo-500/15 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-widest text-indigo-600 dark:text-indigo-300">
              <span className="glow-dot" />
              {t('nav.adminBadge')}
            </span>
          </div>
        </div>

        <main id="main-content" tabIndex={-1} className="flex-1 overflow-auto focus:outline-none">
          <div className="max-w-7xl mx-auto px-4 py-4 md:px-8 md:py-8">
            <PwaInstallBanner />
            <PageTransition>
              <Outlet />
            </PageTransition>
          </div>
        </main>
      </div>
    </div>
  );
}
