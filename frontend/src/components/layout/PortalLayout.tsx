import { Outlet, Link, useLocation, Navigate } from 'react-router-dom';
import { User, FileText, Calendar, Receipt, LogOut, ChevronDown, Bell, Users, Clock, MoreHorizontal, ScanLine } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getNotificationCount } from '@/api/notifications';
import { BrandLogo } from '@/components/ui/BrandLogo';

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
  const { data: notifCount } = useQuery({
    queryKey: ['notification-count'],
    queryFn: getNotificationCount,
    refetchInterval: 30_000,
    enabled: isAuthenticated,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-screen bg-gray-100">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
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
    <div className="min-h-screen bg-gray-100">
      {/* Top Navigation Bar */}
      <header className="bg-white border-b border-gray-200 sticky top-0 z-50">
        <div className="max-w-7xl mx-auto flex items-center justify-between h-14 md:h-16 px-4 md:px-6">
          {/* Left: Logo + nav */}
          <div className="flex items-center gap-8">
            <div className="flex items-center gap-2">
              <BrandLogo variant="lockup-dark" className="h-8 w-auto shrink-0" />
              <span className="bg-gray-100 text-gray-600 text-[10px] font-semibold px-1.5 py-0.5 rounded">
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
                        ? 'text-gray-900 bg-gray-100 font-semibold'
                        : 'text-gray-500 hover:text-gray-900 hover:bg-gray-50'
                    }`}
                  >
                    <item.icon className="w-4 h-4" />
                    <span>{item.name}</span>
                    {item.name === 'Notifications' && hasUnread && (
                      <span className="absolute -top-0.5 -right-0.5 min-w-[18px] h-[18px] px-1 text-[10px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none">
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
              className="relative md:hidden p-2 text-gray-500 hover:text-gray-900 rounded-lg"
            >
              <Bell className="w-5 h-5" />
              {hasUnread && (
                <span className="absolute top-1 right-1 min-w-[16px] h-[16px] px-0.5 text-[9px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none">
                  {notifCount.unread}
                </span>
              )}
            </Link>

            <div className="relative">
              <button
                onClick={() => setShowUserMenu(!showUserMenu)}
                className="flex items-center gap-2.5 text-sm text-gray-600 hover:text-gray-900 transition-all-fast"
              >
                <div className="w-8 h-8 rounded-full bg-black flex items-center justify-center text-xs font-bold text-white">
                  {user?.full_name?.[0] || 'U'}
                </div>
                <span className="hidden md:inline font-medium">{user?.full_name || 'User'}</span>
                <ChevronDown className="hidden md:block w-3.5 h-3.5 text-gray-400" />
              </button>

              {showUserMenu && (
                <>
                  <div className="fixed top-0 left-0 z-10 w-screen h-screen" onClick={() => setShowUserMenu(false)} />
                  <div className="absolute right-0 mt-2 w-52 bg-white rounded-2xl shadow-lg border border-gray-200 z-20 py-1 overflow-hidden">
                    <div className="px-4 py-3 border-b border-gray-100 bg-gray-50">
                      <p className="text-sm font-semibold text-gray-900">{user?.full_name}</p>
                      <p className="text-xs text-gray-500 mt-0.5">{user?.email}</p>
                    </div>
                    <button
                      onClick={logout}
                      className="flex items-center gap-2.5 w-full px-4 py-3 text-sm text-gray-600 hover:bg-gray-50 transition-all-fast"
                    >
                      <LogOut className="w-4 h-4" /> Sign Out
                    </button>
                  </div>
                </>
              )}
            </div>
          </div>
        </div>
      </header>

      {/* Content */}
      <main className="max-w-7xl mx-auto px-4 py-4 md:px-6 md:py-8 pb-20 md:pb-8">
        <Outlet />
      </main>

      {/* Mobile Bottom Tab Bar */}
      <nav className="md:hidden fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200 z-50 safe-area-bottom">
        <div className="flex items-stretch">
          {mobileTabNav.map((item) => {
            const isActive =
              location.pathname === item.href ||
              location.pathname.startsWith(item.href + '/');
            return (
              <Link
                key={item.name}
                to={item.href}
                className={`flex-1 flex flex-col items-center gap-0.5 py-2 pt-2.5 text-[10px] font-medium transition-colors ${
                  isActive ? 'text-gray-900' : 'text-gray-400'
                }`}
              >
                <item.icon className="w-5 h-5" />
                {item.name}
              </Link>
            );
          })}

          {/* More tab */}
          <div className="flex-1 relative">
            <button
              onClick={() => setShowMoreMenu(!showMoreMenu)}
              className={`w-full flex flex-col items-center gap-0.5 py-2 pt-2.5 text-[10px] font-medium transition-colors ${
                isMoreActive ? 'text-gray-900' : 'text-gray-400'
              }`}
            >
              <MoreHorizontal className="w-5 h-5" />
              More
            </button>

            {showMoreMenu && (
              <>
                <div className="fixed inset-0 z-10" onClick={() => setShowMoreMenu(false)} />
                <div className="absolute bottom-full right-0 mb-2 w-48 bg-white rounded-2xl shadow-lg border border-gray-200 z-20 py-1 overflow-hidden">
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
                            ? 'text-gray-900 font-semibold bg-gray-50'
                            : 'text-gray-600 hover:bg-gray-50'
                        }`}
                      >
                        <item.icon className="w-4 h-4" />
                        {item.name}
                        {item.name === 'Notifications' && hasUnread && (
                          <span className="ml-auto min-w-[18px] h-[18px] px-1 text-[10px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none">
                            {notifCount.unread}
                          </span>
                        )}
                      </Link>
                    );
                  })}
                </div>
              </>
            )}
          </div>
        </div>
      </nav>
    </div>
  );
}
