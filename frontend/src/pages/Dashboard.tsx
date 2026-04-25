import { useQuery } from '@tanstack/react-query';
import { Users, DollarSign, Building2, TrendingUp } from 'lucide-react';
import { getDashboardSummary } from '@/api/dashboard';
import { formatMYR } from '@/lib/utils';
import { useAuth } from '@/context/AuthContext';
import { canAccessPayrollData } from '@/lib/roles';

function StatCard({
  title,
  value,
  subtitle,
  icon: Icon,
  color,
  iconBg,
}: {
  title: string;
  value: string;
  subtitle?: string;
  icon: React.ElementType;
  color: string;
  iconBg: string;
}) {
  return (
    <div className="card">
      <div className="flex items-start justify-between">
        <div className="space-y-1">
          <p className="text-sm text-gray-400 font-medium">{title}</p>
          <p className={`text-2xl font-bold ${color}`}>{value}</p>
          {subtitle && <p className="text-xs text-gray-400 mt-1">{subtitle}</p>}
        </div>
        <div className={`w-11 h-11 rounded-xl ${iconBg} flex items-center justify-center`}>
          <Icon className="w-5 h-5 text-white" />
        </div>
      </div>
    </div>
  );
}

export function Dashboard() {
  const { user } = useAuth();
  const canViewPayroll = canAccessPayrollData(user);
  const { data, isLoading } = useQuery({
    queryKey: ['dashboard'],
    queryFn: getDashboardSummary,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <div className="page-header">
        <h1 className="page-title">Dashboard</h1>
        <p className="page-subtitle">
          {canViewPayroll ? "Overview of your company's payroll" : "Overview of your company"}
        </p>
      </div>

      {/* Stats Grid */}
      <div className={`grid grid-cols-1 md:grid-cols-2 ${canViewPayroll ? 'lg:grid-cols-4' : ''} gap-3 md:gap-5`}>
        <StatCard
          title="Total Employees"
          value={String(data?.active_employees ?? 0)}
          subtitle={`${data?.total_employees ?? 0} total (incl. inactive)`}
          icon={Users}
          color="text-gray-900"
          iconBg="bg-black"
        />
        {canViewPayroll && (
          <>
            <StatCard
              title="Last Payroll"
              value={data?.last_payroll_total_net ? formatMYR(data.last_payroll_total_net) : 'N/A'}
              subtitle={data?.last_payroll_period ? `Period: ${data.last_payroll_period}` : 'No payroll run yet'}
              icon={DollarSign}
              color="text-emerald-700"
              iconBg="bg-gradient-to-br from-emerald-500 to-emerald-600"
            />
            <StatCard
              title="YTD Gross"
              value={formatMYR(data?.ytd_total_gross ?? 0)}
              subtitle="Year-to-date total gross"
              icon={TrendingUp}
              color="text-violet-700"
              iconBg="bg-gradient-to-br from-violet-500 to-violet-600"
            />
            <StatCard
              title="YTD Employer Statutory"
              value={formatMYR(
                (data?.ytd_total_epf_employer ?? 0) +
                (data?.ytd_total_socso_employer ?? 0) +
                (data?.ytd_total_eis_employer ?? 0)
              )}
              subtitle="EPF + SOCSO + EIS employer"
              icon={Building2}
              color="text-amber-700"
              iconBg="bg-gradient-to-br from-amber-500 to-amber-600"
            />
          </>
        )}
      </div>

      {/* Department Breakdown */}
      {data?.departments && data.departments.length > 0 && (
        <div className="card">
          <div className="section-header">
            <span className="section-number" style={{ backgroundColor: '#000000' }}>H</span>
            <span className="section-title">Headcount by Department</span>
          </div>
          <div className="space-y-4">
            {data.departments.map((dept) => (
              <div key={dept.department} className="flex items-center justify-between">
                <span className="text-sm text-gray-600 min-w-[140px]">{dept.department}</span>
                <div className="flex items-center gap-4 flex-1 ml-4">
                  <div className="flex-1 bg-gray-100 rounded-full h-2.5">
                    <div
                      className="bg-gray-900 h-2.5 rounded-full transition-all duration-500"
                      style={{ width: `${(dept.count / (data.active_employees || 1)) * 100}%` }}
                    />
                  </div>
                  <span className="text-sm font-semibold text-gray-900 w-8 text-right">{dept.count}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
