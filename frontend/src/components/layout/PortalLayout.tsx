import { Outlet, Link, useLocation, Navigate } from 'react-router-dom';
import { Home, User, FileText, Calendar, Receipt, LogOut, ChevronDown, Bell, Users, Clock } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getNotificationCount } from '@/api/notifications';

const portalNav = [
  { name: 'Home', href: '/portal', icon: Home },
  { name: 'My Profile', href: '/portal/profile', icon: User },
  { name: 'My Payslips', href: '/portal/payslips', icon: FileText },
  { name: 'Leave', href: '/portal/leave', icon: Calendar },
  { name: 'Claims', href: '/portal/claims', icon: Receipt },
  { name: 'Overtime', href: '/portal/overtime', icon: Clock },
  { name: 'Team Calendar', href: '/portal/team-calendar', icon: Users },
  { name: 'Notifications', href: '/portal/notifications', icon: Bell },
];

export function PortalLayout() {
  const { user, logout, isAuthenticated, isLoading } = useAuth();
  const location = useLocation();
  const [showUserMenu, setShowUserMenu] = useState(false);
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

  return (
    <div className="min-h-screen bg-gray-100">
      {/* Top Navigation Bar */}
      <header className="bg-white border-b border-gray-200 sticky top-0 z-50">
        <div className="max-w-7xl mx-auto flex items-center justify-between h-16 px-6">
          {/* Left: Logo + nav */}
          <div className="flex items-center gap-8">
            <div className="flex items-center gap-2.5">
              <div className="w-8 h-8 bg-black rounded-lg flex items-center justify-center">
                <span className="text-white font-bold text-sm">P</span>
              </div>
              <div>
                <span className="font-bold text-gray-900 text-sm">PayrollMY</span>
                <span className="ml-1.5 bg-gray-100 text-gray-600 text-[10px] font-semibold px-1.5 py-0.5 rounded">
                  PORTAL
                </span>
              </div>
            </div>

            <nav className="flex items-center gap-1">
              {portalNav.map((item) => {
                const isActive =
                  location.pathname === item.href ||
                  (item.href !== '/portal' && location.pathname.startsWith(item.href));
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
                    <span className="hidden md:inline">{item.name}</span>
                    {item.name === 'Notifications' && notifCount && notifCount.unread > 0 && (
                      <span className="absolute -top-0.5 -right-0.5 min-w-[18px] h-[18px] px-1 text-[10px] font-bold bg-red-500 text-white rounded-full flex items-center justify-center leading-none">
                        {notifCount.unread}
                      </span>
                    )}
                  </Link>
                );
              })}
            </nav>
          </div>

          {/* Right: User dropdown */}
          <div className="relative">
            <button
              onClick={() => setShowUserMenu(!showUserMenu)}
              className="flex items-center gap-2.5 text-sm text-gray-600 hover:text-gray-900 transition-all-fast"
            >
              <div className="w-8 h-8 rounded-full bg-black flex items-center justify-center text-xs font-bold text-white">
                {user?.full_name?.[0] || 'U'}
              </div>
              <span className="hidden md:inline font-medium">{user?.full_name || 'User'}</span>
              <ChevronDown className="w-3.5 h-3.5 text-gray-400" />
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
                    className="flex items-center gap-2.5 w-full px-4 py-2.5 text-sm text-gray-600 hover:bg-gray-50 transition-all-fast"
                  >
                    <LogOut className="w-4 h-4" /> Sign Out
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </header>

      {/* Content */}
      <main className="max-w-7xl mx-auto px-6 py-8">
        <Outlet />
      </main>
    </div>
  );
}
