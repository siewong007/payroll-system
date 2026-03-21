import { Link, useLocation } from 'react-router-dom';
import {
  LayoutDashboard,
  Building2,
  Users,
  Users2,
  Calculator,
  FileText,
  Settings,
  LogOut,
  ClipboardCheck,
  BarChart3,
  CalendarDays,
  UserCog,
  Shield,
  KeyRound,
} from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { CompanySwitcher } from './CompanySwitcher';

const navigation = [
  { name: 'Dashboard', href: '/', icon: LayoutDashboard, hideFor: ['super_admin'] },
  { name: 'Company', href: '/company', icon: Building2, hideFor: ['super_admin'] },
  { name: 'Employees', href: '/employees', icon: Users, hideFor: ['super_admin'] },
  { name: 'Payroll', href: '/payroll', icon: Calculator, hideFor: ['exec', 'super_admin'] },
  { name: 'Teams', href: '/teams', icon: Users2, hideFor: ['super_admin'] },
  { name: 'Calendar', href: '/calendar', icon: CalendarDays, hideFor: ['super_admin'] },
  { name: 'Approvals', href: '/approvals', icon: ClipboardCheck, hideFor: ['super_admin'] },
  { name: 'Reports', href: '/reports', icon: BarChart3, hideFor: ['super_admin'] },
  { name: 'Documents', href: '/documents', icon: FileText, hideFor: ['super_admin'] },
  { name: 'Settings', href: '/settings', icon: Settings, hideFor: ['super_admin'] },
  { name: 'Companies', href: '/companies', icon: Building2, showFor: ['super_admin'] },
  { name: 'Users', href: '/users', icon: UserCog, showFor: ['super_admin'] },
  { name: 'Roles', href: '/roles', icon: Shield, showFor: ['super_admin'] },
  { name: 'Password Resets', href: '/password-resets', icon: KeyRound, showFor: ['super_admin'] },
];

export function Sidebar() {
  const location = useLocation();
  const { user, logout } = useAuth();

  return (
    <aside className="flex flex-col w-64 min-h-screen bg-white border-r border-gray-200">
      {/* Logo */}
      <div className="p-6 border-b border-gray-100">
        <div className="flex items-center gap-2.5">
          <div className="w-8 h-8 bg-black rounded-lg flex items-center justify-center">
            <span className="text-white font-bold text-sm">P</span>
          </div>
          <div>
            <h1 className="text-base font-bold text-gray-900">PayrollMY</h1>
            <p className="text-[10px] text-gray-400 -mt-0.5 uppercase tracking-wider">Admin Console</p>
          </div>
        </div>
      </div>

      {/* Company Switcher */}
      <CompanySwitcher />

      {/* Navigation */}
      <nav className="flex-1 py-4 px-3 space-y-0.5">
        {navigation.filter((item) => {
          const role = user?.role ?? '';
          if (item.hideFor?.includes(role)) return false;
          if (item.showFor && !item.showFor.includes(role)) return false;
          return true;
        }).map((item) => {
          const isActive = location.pathname === item.href ||
            (item.href !== '/' && location.pathname.startsWith(item.href));
          return (
            <Link
              key={item.name}
              to={item.href}
              className={`flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium transition-all-fast ${
                isActive
                  ? 'bg-gray-100 text-gray-900'
                  : 'text-gray-500 hover:bg-gray-50 hover:text-gray-900'
              }`}
            >
              <item.icon className={`w-[18px] h-[18px] ${isActive ? 'text-gray-900' : ''}`} />
              {item.name}
            </Link>
          );
        })}
      </nav>

      {/* User */}
      <div className="p-4 border-t border-gray-100">
        <div className="flex items-center gap-3 mb-3">
          <div className="w-9 h-9 rounded-full bg-black flex items-center justify-center text-sm font-semibold text-white">
            {user?.full_name?.[0] || 'U'}
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-semibold text-gray-900 truncate">{user?.full_name || 'User'}</p>
            <p className="text-xs text-gray-400 truncate capitalize">{user?.role?.replace('_', ' ')}</p>
          </div>
        </div>
        <button
          onClick={logout}
          className="flex items-center gap-2 text-sm text-gray-400 hover:text-gray-700 transition-all-fast w-full px-1"
        >
          <LogOut className="w-4 h-4" />
          Sign Out
        </button>
      </div>
    </aside>
  );
}
