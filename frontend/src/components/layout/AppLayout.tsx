import { useState } from 'react';
import { Outlet, Navigate } from 'react-router';
import { Menu } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { Sidebar } from './Sidebar';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { PageTransition } from '@/components/ui/PageTransition';
import { hasOnlyEmployeeRole } from '@/lib/roles';

export function AppLayout() {
  const { user, isAuthenticated, isLoading } = useAuth();
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
      {/* Ambient aurora behind the whole console */}
      <div aria-hidden className="pointer-events-none absolute inset-0 -z-10 overflow-hidden">
        <div className="ambient-blob animate-float-a -top-32 left-1/4 h-96 w-96 bg-indigo-300/40" />
        <div className="ambient-blob animate-float-b top-1/3 -right-40 h-[28rem] w-[28rem] bg-violet-300/30" />
        <div className="ambient-blob animate-float-a -bottom-32 left-1/2 h-80 w-80 bg-sky-300/30" />
      </div>

      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />

      <div className="flex-1 flex flex-col min-w-0">
        {/* Mobile top bar */}
        <div className="md:hidden glass-nav sticky top-0 z-30 flex items-center gap-3 px-4 py-3">
          <button
            onClick={() => setSidebarOpen(true)}
            className="p-2 -ml-2 text-gray-600 hover:text-gray-900 hover:bg-white/60 rounded-lg transition-all-fast"
          >
            <Menu className="w-5 h-5" />
          </button>
          <BrandLogo variant="lockup-dark" className="h-7 w-auto" />
          <span className="ml-auto inline-flex items-center gap-1.5 rounded-full border border-indigo-200/60 bg-indigo-50/80 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-widest text-indigo-600">
            <span className="glow-dot" />
            Admin
          </span>
        </div>

        <main className="flex-1 overflow-auto">
          <div className="max-w-7xl mx-auto px-4 py-4 md:px-8 md:py-8">
            <PageTransition>
              <Outlet />
            </PageTransition>
          </div>
        </main>
      </div>
    </div>
  );
}
