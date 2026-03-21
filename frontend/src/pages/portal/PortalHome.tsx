import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { User, FileText, Calendar, Receipt, ChevronRight, Sparkles } from 'lucide-react';
import { getMyProfile } from '@/api/portal';
import { useAuth } from '@/context/AuthContext';

export function PortalHome() {
  const { user } = useAuth();
  const { data: profile } = useQuery({
    queryKey: ['my-profile'],
    queryFn: getMyProfile,
  });

  const quickLinks = [
    { name: 'My Profile', desc: 'View and update your personal details', href: '/portal/profile', icon: User, color: 'from-blue-500 to-blue-600', bg: 'bg-gray-100' },
    { name: 'My Payslips', desc: 'View your salary slips and payment history', href: '/portal/payslips', icon: FileText, color: 'from-emerald-500 to-emerald-600', bg: 'bg-emerald-50' },
    { name: 'Leave', desc: 'Check your leave balance and apply for leave', href: '/portal/leave', icon: Calendar, color: 'from-violet-500 to-violet-600', bg: 'bg-violet-50' },
    { name: 'Claims', desc: 'Submit expense claims and track status', href: '/portal/claims', icon: Receipt, color: 'from-amber-500 to-amber-600', bg: 'bg-amber-50' },
  ];

  return (
    <div className="space-y-8">
      {/* Welcome Banner */}
      <div className="bg-white rounded-2xl shadow p-8 relative overflow-hidden">
        <div className="absolute top-0 right-0 w-64 h-64 bg-gradient-to-bl from-gray-50 to-transparent rounded-bl-full opacity-60" />
        <div className="relative">
          <div className="flex items-center gap-2 mb-1">
            <Sparkles className="w-4 h-4 text-gray-400" />
            <span className="text-sm text-gray-500 font-medium">Welcome back</span>
          </div>
          <h1 className="text-2xl font-bold text-gray-900">
            {profile?.full_name || user?.full_name || 'Employee'}
          </h1>
          <p className="text-gray-500 mt-1 text-sm">
            {profile?.designation || 'Employee'} &middot; {profile?.department || 'General'}
          </p>
          {profile && (
            <div className="flex gap-6 mt-4">
              <div className="text-sm">
                <span className="text-gray-400">Employee No</span>
                <p className="font-semibold text-gray-700">{profile.employee_number}</p>
              </div>
              <div className="text-sm">
                <span className="text-gray-400">Date Joined</span>
                <p className="font-semibold text-gray-700">{new Date(profile.date_joined).toLocaleDateString('en-MY', { day: 'numeric', month: 'long', year: 'numeric' })}</p>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Quick Links */}
      <div>
        <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4">Quick Access</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {quickLinks.map((link) => (
            <Link
              key={link.name}
              to={link.href}
              className="bg-white rounded-2xl shadow p-6 hover:shadow-md transition-all-fast group"
            >
              <div className="flex items-start justify-between">
                <div className="flex items-start gap-4">
                  <div className={`w-11 h-11 rounded-xl bg-gradient-to-br ${link.color} flex items-center justify-center shadow-sm`}>
                    <link.icon className="w-5 h-5 text-white" />
                  </div>
                  <div>
                    <h3 className="font-semibold text-gray-900">{link.name}</h3>
                    <p className="text-sm text-gray-400 mt-0.5">{link.desc}</p>
                  </div>
                </div>
                <ChevronRight className="w-5 h-5 text-gray-300 group-hover:text-gray-500 group-hover:translate-x-0.5 transition-all-fast mt-1" />
              </div>
            </Link>
          ))}
        </div>
      </div>
    </div>
  );
}
