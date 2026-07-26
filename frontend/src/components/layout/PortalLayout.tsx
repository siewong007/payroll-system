import { Outlet, Link, useLocation, Navigate } from 'react-router';
import { User, FileText, Calendar, Receipt, LogOut, ChevronDown, Bell, Users, Clock, MoreHorizontal, ScanLine, Sparkles } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { useEffect, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { AnimatePresence, motion } from 'framer-motion';
import { getNotificationCount } from '@/api/notifications';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { PageTransition } from '@/components/ui/PageTransition';

const portalNav = [
  { name: 'My Profile', href: '/portal/profile', icon: User },
  { name: 'My Payslips', href: '/portal/payslips', icon: FileText },
  { name: 'Leave', href: '/portal/leave', icon: Calendar },
  { name: 'Attendance', href: '/portal/attendance', icon: ScanLine },
  { name: 'Claims', href: '/portal/claims', icon: Receipt },
  { name: 'Overtime', href: '/portal/overtime', icon: Clock },
  { name: 'Team Calendar', href: '/portal/team-calendar', icon: Users },
  { name: 'Notifications', href: '/portal/notifications', icon: Bell },
];

// Primary tabs shown in mobile bottom bar
const mobileTabNav = [
  { name: 'Profile', href: '/portal/profile', icon: User },
  { name: 'Payslips', href: '/portal/payslips', icon: FileText },
  { name: 'Leave', href: '/portal/leave', icon: Calendar },
  { name: 'Claims', href: '/portal/claims', icon: Receipt },
];

// Items shown in "More" menu on mobile
const mobileMoreNav = [
  { name: 'Attendance', href: '/portal/attendance', icon: ScanLine },
  { name: 'Overtime', href: '/portal/overtime', icon: Clock },
  { name: 'Team Calendar', href: '/portal/team-calendar', icon: Users },
  { name: 'Notifications', href: '/portal/notifications', icon: Bell },
];

export function PortalLayout() {
  const { user, logout, isAuthenticated, isLoading } = useAuth();
  const location = useLocation();
  const [showUserMenu, setShowUserMenu] = useState(false);
  const [showMoreMenu, setShowMoreMenu] = useState(false);
  const userMenuRef = useRef<HTMLDivElement>(null);
  const moreMenuRef = useRef<HTMLDivElement>(null);
  const { data: notifCount } = useQuery({
    queryKey: ['notification-count'],
    queryFn: getNotificationCount,
    refetchInterval: 30_000,
    enabled: isAuthenticated,
  });

  // Close dropdowns on outside click (same idiom as CompanySwitcher).
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (userMenuRef.current && !userMenuRef.current.contains(e.target as Node)) {
        setShowUserMenu(false);
      }
      if (moreMenuRef.current && !moreMenuRef.current.contains(e.target as Node)) {
        setShowMoreMenu(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
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

  const isMoreActive = mobileMoreNav.some(
    (item) => location.pathname === item.href || location.pathname.startsWith(item.href)
  );

  const hasUnread = notifCount && notifCount.unread > 0;

  return (
    <div className="theme-portal relative isolate min-h-screen bg-[var(--background)]">
      {/* Ambient aurora behind the portal */}
      <div aria-hidden className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
        <div className="ambient-blob animate-float-a -top-32 -left-24 h-96 w-96 bg-teal-200/50" />
        <div className="ambient-blob animate-float-b top-1/4 -right-32 h-[26rem] w-[26rem] bg-emerald-200/40" />
        <div className="ambient-blob animate-float-a -bottom-32 left-1/3 h-80 w-80 bg-sky-200/40" />
      </div>

      {/* Top Navigation Bar */}
      <header className="glass-nav sticky top-0 z-50">
        <div className="max-w-7xl mx-auto flex items-center justify-between h-14 md:h-16 px-4 md:px-6">
          {/* Left: Logo + nav */}
          <div className="flex items-center gap-8">
            <div className="flex items-center gap-2">
              <BrandLogo variant="lockup-dark" className="h-8 w-auto shrink-0" />
              <span className="inline-flex items-center gap-1 bg-gradient-to-r from-teal-500 to-emerald-500 text-white text-[10px] font-semibold px-2 py-0.5 rounded-full shadow-[0_2px_10px_-2px_var(--glow)]">
                <Sparkles className="w-2.5 h-2.5" />
                PORTAL
              </span>
            </div>

            {/* Desktop nav */}
            <nav className="hidden md:flex items-center gap-1">
              {portalNav.map((item) => {
                const isActive =
                  location.pathname === item.href ||
                  location.pathname.startsWith(item.href + '/');
                return (
                  <Link
                    key={item.name}
                    to={item.href}
                    className={`relative flex items-center gap-1.5 px-3 py-2 text-sm rounded-xl transition-all-fast ${
                      isActive
                        ? 'text-teal-900 font-semibold'
                        : 'text-gray-500 hover:text-gray-900 hover:bg-white/60'
                    }`}
                  >
                    {isActive && (
                      <motion.span
                        layoutId="portal-nav-active"
                        className="absolute inset-0 rounded-xl bg-gradient-to-r from-teal-500/15 to-emerald-500/15 ring-1 ring-teal-500/25 shadow-[0_4px_14px_-6px_var(--glow)]"
                        transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                      />
                    )}
                    <item.icon className={`relative w-4 h-4 ${isActive ? 'text-teal-600' : ''}`} />
                    <span className="relative">{item.name}</span>
                    {item.name === 'Notifications' && hasUnread && (
                      <span className="absolute -top-0.5 -right-0.5 min-w-[18px] h-[18px] px-1 text-[10px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none shadow-[0_0_10px_rgba(239,68,68,0.6)]">
                        {notifCount.unread}
                      </span>
                    )}
                  </Link>
                );
              })}
            </nav>
          </div>

          {/* Right: Mobile notification bell + User dropdown */}
          <div className="flex items-center gap-2">
            {/* Mobile notification bell */}
            <Link
              to="/portal/notifications"
              className="relative md:hidden p-2 text-gray-500 hover:text-gray-900 hover:bg-white/60 rounded-lg transition-all-fast"
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
                onClick={() => setShowUserMenu(!showUserMenu)}
                className="flex items-center gap-2.5 text-sm text-gray-600 hover:text-gray-900 transition-all-fast"
              >
                <div className="w-8 h-8 rounded-full bg-gradient-to-br from-teal-500 to-emerald-500 ring-2 ring-white/80 shadow-[0_4px_14px_-4px_var(--glow)] flex items-center justify-center text-xs font-bold text-white">
                  {user?.full_name?.[0] || 'U'}
                </div>
                <span className="hidden md:inline font-medium">{user?.full_name || 'User'}</span>
                <ChevronDown
                  className={`hidden md:block w-3.5 h-3.5 text-gray-400 transition-transform duration-200 ${showUserMenu ? 'rotate-180' : ''}`}
                />
              </button>

              <AnimatePresence>
                {showUserMenu && (
                  <motion.div
                    initial={{ opacity: 0, y: -6, scale: 0.97 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    exit={{ opacity: 0, y: -6, scale: 0.97 }}
                    transition={{ duration: 0.16, ease: 'easeOut' }}
                    className="glass-menu absolute right-0 mt-2 w-52 rounded-2xl z-20 py-1 overflow-hidden"
                  >
                    <div className="px-4 py-3 border-b border-gray-200/60">
                      <p className="text-sm font-semibold text-gray-900">{user?.full_name}</p>
                      <p className="text-xs text-gray-500 mt-0.5">{user?.email}</p>
                    </div>
                    <button
                      onClick={logout}
                      className="flex items-center gap-2.5 w-full px-4 py-3 text-sm text-gray-600 hover:bg-white/70 hover:text-red-600 transition-all-fast"
                    >
                      <LogOut className="w-4 h-4" /> Sign Out
                    </button>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </div>
        </div>
      </header>

      {/* Content */}
      {/* The bottom bar is ~56px plus env(safe-area-inset-bottom) (~34px on a
          phone with a home indicator), so a flat pb-20 leaves the last row of
          content under it on exactly the devices employees check in from. */}
      <main className="max-w-7xl mx-auto px-4 py-4 md:px-6 md:py-8 pb-[calc(5rem+env(safe-area-inset-bottom))] md:pb-8">
        <PageTransition>
          <Outlet />
        </PageTransition>
      </main>

      {/* Mobile Bottom Tab Bar */}
      <nav className="md:hidden fixed bottom-0 left-0 right-0 z-50 safe-area-bottom bg-white/80 backdrop-blur-xl border-t border-white/60 shadow-[0_-4px_24px_-12px_var(--glow)]">
        <div className="flex items-stretch">
          {mobileTabNav.map((item) => {
            const isActive =
              location.pathname === item.href ||
              location.pathname.startsWith(item.href + '/');
            return (
              <Link
                key={item.name}
                to={item.href}
                onClick={() => setShowMoreMenu(false)}
                className={`relative z-20 flex-1 flex flex-col items-center gap-0.5 py-2 pt-2.5 text-[10px] font-medium transition-colors ${
                  isActive ? 'text-teal-600' : 'text-gray-400'
                }`}
              >
                {isActive && (
                  <motion.span
                    layoutId="portal-tab-active"
                    className="absolute top-0 h-[3px] w-8 rounded-b-full bg-gradient-to-r from-teal-500 to-emerald-500 shadow-[0_2px_8px_var(--glow)]"
                    transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                  />
                )}
                <item.icon className="w-5 h-5" />
                {item.name}
              </Link>
            );
          })}

          {/* More tab */}
          <div ref={moreMenuRef} className="flex-1 relative">
            <button
              onClick={() => setShowMoreMenu(!showMoreMenu)}
              className={`relative w-full flex flex-col items-center gap-0.5 py-2 pt-2.5 text-[10px] font-medium transition-colors ${
                isMoreActive ? 'text-teal-600' : 'text-gray-400'
              }`}
            >
              {isMoreActive && (
                <motion.span
                  layoutId="portal-tab-active"
                  className="absolute top-0 h-[3px] w-8 rounded-b-full bg-gradient-to-r from-teal-500 to-emerald-500 shadow-[0_2px_8px_var(--glow)]"
                  transition={{ type: 'spring', stiffness: 420, damping: 34 }}
                />
              )}
              <MoreHorizontal className="w-5 h-5" />
              More
            </button>

            <AnimatePresence>
              {showMoreMenu && (
                  <motion.div
                    initial={{ opacity: 0, y: 8, scale: 0.97 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    exit={{ opacity: 0, y: 8, scale: 0.97 }}
                    transition={{ duration: 0.16, ease: 'easeOut' }}
                    className="glass-menu absolute bottom-full right-0 mb-2 w-48 rounded-2xl z-20 py-1 overflow-hidden"
                  >
                    {mobileMoreNav.map((item) => {
                      const isActive =
                        location.pathname === item.href ||
                        location.pathname.startsWith(item.href);
                      return (
                        <Link
                          key={item.name}
                          to={item.href}
                          onClick={() => setShowMoreMenu(false)}
                          className={`flex items-center gap-3 px-4 py-3 text-sm transition-colors ${
                            isActive
                              ? 'text-teal-800 font-semibold bg-teal-500/10'
                              : 'text-gray-600 hover:bg-white/70'
                          }`}
                        >
                          <item.icon className={`w-4 h-4 ${isActive ? 'text-teal-600' : ''}`} />
                          {item.name}
                          {item.name === 'Notifications' && hasUnread && (
                            <span className="ml-auto min-w-[18px] h-[18px] px-1 text-[10px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none">
                              {notifCount.unread}
                            </span>
                          )}
                        </Link>
                      );
                    })}
                  </motion.div>
              )}
            </AnimatePresence>
          </div>
        </div>
      </nav>
    </div>
  );
}
