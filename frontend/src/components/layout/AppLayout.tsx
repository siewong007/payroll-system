import { useState } from 'react';
import { Outlet, Navigate } from 'react-router-dom';
import { Menu } from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { Sidebar } from './Sidebar';

export function AppLayout() {
  const { user, isAuthenticated, isLoading } = useAuth();
  const [sidebarOpen, setSidebarOpen] = useState(false);

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

  if (user?.role === 'employee') {
    return <Navigate to="/portal" replace />;
  }

  return (
    <div className="flex h-screen overflow-hidden bg-gray-100">
      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />

      <div className="flex-1 flex flex-col min-w-0">
        {/* Mobile top bar */}
        <div className="md:hidden flex items-center gap-3 px-4 py-3 bg-white border-b border-gray-200">
          <button
            onClick={() => setSidebarOpen(true)}
            className="p-2 -ml-2 text-gray-600 hover:text-gray-900 rounded-lg"
          >
            <Menu className="w-5 h-5" />
          </button>
          <div className="flex items-center gap-2">
            <div className="w-7 h-7 bg-black rounded-lg flex items-center justify-center">
              <span className="text-white font-bold text-xs">P</span>
            </div>
            <span className="text-sm font-bold text-gray-900">PayrollMY</span>
          </div>
        </div>

        <main className="flex-1 overflow-auto">
          <div className="max-w-7xl mx-auto px-4 py-4 md:px-8 md:py-8">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
}
