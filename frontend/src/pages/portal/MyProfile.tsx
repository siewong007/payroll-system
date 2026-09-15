import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { User, MapPin, Briefcase, Shield, CreditCard } from 'lucide-react';
import { getMyProfile } from '@/api/portal';
import { formatMYR, formatDate } from '@/lib/utils';
import { PasskeyManagement } from '@/components/PasskeyManagement';
import { SessionManagement } from '@/components/SessionManagement';
import { LinkedAccounts } from '@/components/LinkedAccounts';
import { CheckInCard } from '@/components/attendance/CheckInCard';

type Profile = NonNullable<Awaited<ReturnType<typeof getMyProfile>>>;

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

function ProfileDetails({ profile }: { profile: Profile }) {
  const { t } = useTranslation();
  const enumLabel = (group: string, value: string | null | undefined) =>
    value ? t(`enums.${group}.${value}`, { defaultValue: value }) : null;
  return (
    <>
      <div className="page-header">
        <p className="page-subtitle">{t('portal.profile.subtitle')}</p>
        <h1 className="page-title">{profile.full_name}</h1>
      </div>

      <div className="p-3 bg-amber-50 text-amber-700 text-sm rounded-xl border border-amber-100">
        {t('portal.profile.managedByAdmin')}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Personal Information */}
        <div className="card">
          <SectionIcon icon={User} label={t('employees.form.personalInfo')} num={1} />
          <div className="divide-y divide-gray-100">
            <Field label={t('portal.profile.fullName')} value={profile.full_name} />
            <Field label={t('employees.form.icNumber')} value={profile.ic_number} />
            <Field label={t('portal.profile.passportNumber')} value={profile.passport_number} />
            <Field label={t('employees.fields.dateOfBirth')} value={profile.date_of_birth ? formatDate(profile.date_of_birth) : null} />
            <Field label={t('employees.fields.gender')} value={enumLabel('gender', profile.gender)} />
            <Field label={t('employees.fields.race')} value={enumLabel('race', profile.race)} />
            <Field label={t('employees.fields.nationality')} value={profile.nationality} />
            <Field label={t('employees.fields.maritalStatus')} value={enumLabel('maritalStatus', profile.marital_status)} />
            <Field label={t('employees.fields.phone')} value={profile.phone} />
            <Field label={t('employees.fields.email')} value={profile.email} />
          </div>
        </div>

        {/* Address + Employment */}
        <div className="space-y-6">
          <div className="card">
            <SectionIcon icon={MapPin} label={t('portal.profile.address')} num={2} />
            <div className="divide-y divide-gray-100">
              <Field label={t('employees.form.addressLine1')} value={profile.address_line1} />
              <Field label={t('employees.form.addressLine2')} value={profile.address_line2} />
              <Field label={t('employees.form.city')} value={profile.city} />
              <Field label={t('employees.form.state')} value={profile.state} />
              <Field label={t('employees.form.postcode')} value={profile.postcode} />
            </div>
          </div>

          <div className="card">
            <SectionIcon icon={Briefcase} label={t('employees.form.employmentDetails')} num={3} />
            <div className="divide-y divide-gray-100">
              <Field label={t('employees.form.employeeNumber')} value={profile.employee_number} />
              <Field label={t('employees.fields.department')} value={profile.department} />
              <Field label={t('employees.fields.designation')} value={profile.designation} />
              <Field label={t('employees.fields.employmentType')} value={enumLabel('employmentType', profile.employment_type)} />
              <Field label={t('employees.fields.dateJoined')} value={formatDate(profile.date_joined)} />
              <Field label={t('employees.fields.confirmationDate')} value={profile.confirmation_date ? formatDate(profile.confirmation_date) : null} />
              {profile.date_resigned && (
                <Field label={t('portal.profile.resignDate')} value={formatDate(profile.date_resigned)} />
              )}
            </div>
          </div>
        </div>

        {/* Statutory + Banking */}
        <div className="space-y-6">
          <div className="card">
            <SectionIcon icon={Shield} label={t('portal.profile.statutoryDetails')} num={4} />
            <div className="divide-y divide-gray-100">
              <Field label={t('portal.profile.immigrationStatus')} value={enumLabel('residency', profile.residency_status)} />
              <Field label={t('employees.fields.epfNumber')} value={profile.epf_number} />
              <Field label={t('employees.fields.epfCategory')} value={enumLabel('epfCategory', profile.epf_category)} />
              <Field label={t('portal.profile.tin')} value={profile.tax_identification_number} />
              <Field label={t('employees.fields.socsoNumber')} value={profile.socso_number} />
              <Field label={t('employees.fields.eisNumber')} value={profile.eis_number} />
              <Field label={t('employees.fields.workingSpouse')} value={profile.working_spouse ? t('common.yes') : t('common.no')} />
              <Field label={t('employees.fields.children')} value={String(profile.num_children ?? 0)} />
              <Field label={t('employees.fields.muslim')} value={profile.is_muslim ? t('common.yes') : t('common.no')} />
              {profile.zakat_eligible && (
                <Field label={t('employees.fields.zakatMonthly')} value={formatMYR(profile.zakat_monthly_amount ?? 0)} />
              )}
            </div>
          </div>

          <div className="card">
            <SectionIcon icon={CreditCard} label={t('employees.form.bankingDetails')} num={5} />
            <div className="divide-y divide-gray-100">
              <Field label={t('employees.form.bankName')} value={profile.bank_name} />
              <Field label={t('employees.form.accountNumber')} value={profile.bank_account_number} />
            </div>
          </div>

          <PasskeyManagement />
          <LinkedAccounts />
          <SessionManagement />
        </div>
      </div>
    </>
  );
}

export function MyProfile() {
  const { t } = useTranslation();
  const { data: profile, isLoading } = useQuery({
    queryKey: ['my-profile'],
    queryFn: getMyProfile,
  });

  // The check-in card renders before — and independently of — the profile
  // fetch. This is the portal's landing screen, so gating it behind an
  // unrelated request would put a spinner between the employee and the one
  // action they open the app to perform twice a day.
  return (
    <div className="space-y-6">
      <CheckInCard />

      {isLoading ? (
        <div className="flex items-center justify-center h-64">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
        </div>
      ) : !profile ? (
        <div className="text-center py-12 text-gray-400">{t('portal.profile.notFound')}</div>
      ) : (
        <ProfileDetails profile={profile} />
      )}
    </div>
  );
}
