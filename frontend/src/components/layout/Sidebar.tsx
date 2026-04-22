import { Link, useLocation } from 'react-router-dom';
import {
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
  Mail,
  DatabaseBackup,
  ScrollText,
  X,
  ScanLine,
} from 'lucide-react';
import { useAuth } from '@/context/AuthContext';
import { CompanySwitcher } from './CompanySwitcher';
import { AnimatePresence, motion } from 'framer-motion';
import { BrandLogo } from '@/components/ui/BrandLogo';

const navigation = [
  { name: 'Company', href: '/company', icon: Building2, hideFor: ['super_admin'] },
  { name: 'Employees', href: '/employees', icon: Users, hideFor: ['super_admin'] },
  { name: 'Payroll', href: '/payroll', icon: Calculator, showFor: ['super_admin', 'payroll_admin'] },
  { name: 'Teams', href: '/teams', icon: Users2, hideFor: ['super_admin'] },
  { name: 'Calendar', href: '/calendar', icon: CalendarDays, hideFor: ['super_admin'] },
  { name: 'Attendance', href: '/attendance', icon: ScanLine, hideFor: ['super_admin'] },
  { name: 'Approvals', href: '/approvals', icon: ClipboardCheck, hideFor: ['super_admin'] },
  { name: 'Reports', href: '/reports', icon: BarChart3, hideFor: ['exec'] },
  { name: 'Documents', href: '/documents', icon: FileText, hideFor: ['super_admin'] },
  { name: 'Letters', href: '/letters', icon: Mail, hideFor: ['super_admin'] },
  { name: 'Settings', href: '/settings', icon: Settings, hideFor: ['super_admin'] },
  { name: 'Companies', href: '/companies', icon: Building2, showFor: ['super_admin'] },
  { name: 'Users', href: '/users', icon: UserCog, showFor: ['super_admin'] },
  { name: 'Roles', href: '/roles', icon: Shield, showFor: ['super_admin'] },
  { name: 'Attendance Settings', href: '/admin/attendance-settings', icon: ScanLine, showFor: ['super_admin'] },
  { name: 'Audit Trail', href: '/audit-trail', icon: ScrollText, showFor: ['super_admin', 'admin', 'hr_manager'] },
  { name: 'Backup', href: '/backup', icon: DatabaseBackup, showFor: ['super_admin', 'admin'] },
];

interface SidebarProps {
  open?: boolean;
  onClose?: () => void;
}

function SidebarContent({ onClose }: { onClose?: () => void }) {
  const location = useLocation();
  const { user, logout } = useAuth();

  return (
    <aside className="flex flex-col w-64 h-full bg-white border-r border-gray-200">
      {/* Logo */}
      <div className="p-6 border-b border-gray-100 flex items-center justify-between">
        <div className="min-w-0">
          <BrandLogo variant="lockup-dark" className="h-8 w-auto" />
          <div>
            <p className="text-[10px] text-gray-400 mt-1 uppercase tracking-wider">Admin Console</p>
          </div>
        </div>
        {onClose && (
          <button
            onClick={onClose}
            className="md:hidden p-2 -mr-2 text-gray-400 hover:text-gray-600 rounded-lg"
          >
            <X className="w-5 h-5" />
          </button>
        )}
      </div>

      {/* Company Switcher */}
      <CompanySwitcher />

      {/* Navigation */}
      <nav className="flex-1 py-4 px-3 space-y-0.5 overflow-y-auto">
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
              onClick={onClose}
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
      <div className="p-4 border-t border-gray-100 shrink-0" style={{ paddingBottom: 'max(1rem, env(safe-area-inset-bottom))' }}>
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-full bg-black flex items-center justify-center text-sm font-semibold text-white shrink-0">
            {user?.full_name?.[0] || 'U'}
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-semibold text-gray-900 truncate">{user?.full_name || 'User'}</p>
            <p className="text-xs text-gray-400 truncate capitalize">{user?.role?.replace('_', ' ')}</p>
          </div>
          <button
            onClick={logout}
            title="Sign Out"
            className="p-2 text-gray-400 hover:text-gray-700 hover:bg-gray-100 rounded-lg transition-all-fast shrink-0"
          >
            <LogOut className="w-4 h-4" />
          </button>
        </div>
      </div>
    </aside>
  );
}

export function Sidebar({ open, onClose }: SidebarProps) {
  return (
    <>
      {/* Desktop sidebar */}
      <div className="hidden md:flex">
        <SidebarContent />
      </div>

      {/* Mobile drawer */}
      <AnimatePresence>
        {open && (
          <>
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="fixed inset-0 bg-black/40 z-40 md:hidden"
              onClick={onClose}
            />
            <motion.div
              initial={{ x: '-100%' }}
              animate={{ x: 0 }}
              exit={{ x: '-100%' }}
              transition={{ type: 'spring', damping: 25, stiffness: 300 }}
              className="fixed inset-y-0 left-0 z-50 md:hidden h-[100dvh]"
            >
              <SidebarContent onClose={onClose} />
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </>
  );
}
