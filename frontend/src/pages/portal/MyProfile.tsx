import { useQuery } from '@tanstack/react-query';
import { User, MapPin, Briefcase, Shield, CreditCard } from 'lucide-react';
import { getMyProfile } from '@/api/portal';
import { formatMYR, formatDate } from '@/lib/utils';
import { PasskeyManagement } from '@/components/PasskeyManagement';

const Field = ({ label, value }: { label: string; value: string | null | undefined }) => (
  <div className="py-3 flex items-start justify-between gap-4">
    <span className="text-sm text-gray-400 min-w-[140px] shrink-0">{label}</span>
    <span className="text-sm font-medium text-gray-900 text-right">{value || '—'}</span>
  </div>
);

const SectionIcon = ({ icon: Icon, label, num }: { icon: React.ElementType; label: string; num: number }) => (
  <div className="section-header">
    <span className="section-number">{num}</span>
    <div className="flex items-center gap-2">
      <Icon className="w-4 h-4 text-gray-400" />
      <span className="section-title">{label}</span>
    </div>
  </div>
);

export function MyProfile() {
  const { data: profile, isLoading } = useQuery({
    queryKey: ['my-profile'],
    queryFn: getMyProfile,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  if (!profile) return <div className="text-center py-12 text-gray-400">Profile not found</div>;

  return (
    <div className="space-y-6">
      <div className="page-header">
        <p className="page-subtitle">Employee Profile</p>
        <h1 className="page-title">{profile.full_name}</h1>
      </div>

      <div className="p-3 bg-amber-50 text-amber-700 text-sm rounded-xl border border-amber-100">
        Profile details are managed by your administrator. Contact HR for any changes.
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Personal Information */}
        <div className="card">
          <SectionIcon icon={User} label="Personal Information" num={1} />
          <div className="divide-y divide-gray-100">
            <Field label="Full Name" value={profile.full_name} />
            <Field label="NRIC / IC Number" value={profile.ic_number} />
            <Field label="Passport Number" value={profile.passport_number} />
            <Field label="Date of Birth" value={profile.date_of_birth ? formatDate(profile.date_of_birth) : null} />
            <Field label="Gender" value={profile.gender} />
            <Field label="Race" value={profile.race} />
            <Field label="Nationality" value={profile.nationality} />
            <Field label="Marital Status" value={profile.marital_status} />
            <Field label="Phone" value={profile.phone} />
            <Field label="Email" value={profile.email} />
          </div>
        </div>

        {/* Address + Employment */}
        <div className="space-y-6">
          <div className="card">
            <SectionIcon icon={MapPin} label="Address" num={2} />
            <div className="divide-y divide-gray-100">
              <Field label="Address Line 1" value={profile.address_line1} />
              <Field label="Address Line 2" value={profile.address_line2} />
              <Field label="City" value={profile.city} />
              <Field label="State" value={profile.state} />
              <Field label="Postcode" value={profile.postcode} />
            </div>
          </div>

          <div className="card">
            <SectionIcon icon={Briefcase} label="Employment Details" num={3} />
            <div className="divide-y divide-gray-100">
              <Field label="Employee Number" value={profile.employee_number} />
              <Field label="Department" value={profile.department} />
              <Field label="Designation" value={profile.designation} />
              <Field label="Employment Type" value={profile.employment_type?.replace('_', ' ')} />
              <Field label="Date Joined" value={formatDate(profile.date_joined)} />
              <Field label="Confirmation Date" value={profile.confirmation_date ? formatDate(profile.confirmation_date) : null} />
              {profile.date_resigned && (
                <Field label="Resign Date" value={formatDate(profile.date_resigned)} />
              )}
            </div>
          </div>
        </div>

        {/* Statutory + Banking */}
        <div className="space-y-6">
          <div className="card">
            <SectionIcon icon={Shield} label="Statutory Details" num={4} />
            <div className="divide-y divide-gray-100">
              <Field label="Immigration Status" value={profile.residency_status?.replace('_', ' ')} />
              <Field label="EPF Number" value={profile.epf_number} />
              <Field label="EPF Category" value={profile.epf_category} />
              <Field label="TIN (Income Tax)" value={profile.tax_identification_number} />
              <Field label="SOCSO Number" value={profile.socso_number} />
              <Field label="EIS Number" value={profile.eis_number} />
              <Field label="Working Spouse" value={profile.working_spouse ? 'Yes' : 'No'} />
              <Field label="Children" value={String(profile.num_children ?? 0)} />
              <Field label="Muslim" value={profile.is_muslim ? 'Yes' : 'No'} />
              {profile.zakat_eligible && (
                <Field label="Zakat (Monthly)" value={formatMYR(profile.zakat_monthly_amount ?? 0)} />
              )}
            </div>
          </div>

          <div className="card">
            <SectionIcon icon={CreditCard} label="Banking Details" num={5} />
            <div className="divide-y divide-gray-100">
              <Field label="Bank Name" value={profile.bank_name} />
              <Field label="Account Number" value={profile.bank_account_number} />
            </div>
          </div>

          <PasskeyManagement />
        </div>
      </div>
    </div>
  );
}
