import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { User, MapPin, Briefcase, Shield, CreditCard, Eye, EyeOff } from 'lucide-react';
import { getMyProfile } from '@/api/portal';
import { formatMYR, formatDate } from '@/lib/utils';
import { PasskeyManagement } from '@/components/PasskeyManagement';
import { SessionManagement } from '@/components/SessionManagement';
import { LinkedAccounts } from '@/components/LinkedAccounts';
import { AppInstallCard } from '@/components/pwa/AppInstallCard';
import { NotificationActivationCard } from '@/components/pwa/NotificationActivationCard';
import { CheckInCard } from '@/components/attendance/CheckInCard';
import { Skeleton } from '@/components/ui/Skeleton';

type Profile = NonNullable<Awaited<ReturnType<typeof getMyProfile>>>;
type MobileTab = 'personal' | 'statutory' | 'security';

function maskSensitive(value: string | null | undefined, revealed: boolean): string | null | undefined {
  if (!value || revealed) return value;
  const trimmed = value.trim();
  if (trimmed.length <= 4) return '••••';
  return `•••• •••• ${trimmed.slice(-4)}`;
}

const Field = ({ label, value }: { label: string; value: string | null | undefined }) => (
  <div className="py-3 flex items-start justify-between gap-4">
    <span className="text-sm text-gray-400 dark:text-slate-400 min-w-[140px] shrink-0">{label}</span>
    <span className="text-sm font-medium text-gray-900 dark:text-slate-100 text-right tabular-nums">{value || '—'}</span>
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
  const [mobileTab, setMobileTab] = useState<MobileTab>('personal');
  const [revealed, setRevealed] = useState(false);

  const enumLabel = (group: string, value: string | null | undefined) =>
    value ? t(`enums.${group}.${value}`, { defaultValue: value }) : null;

  const tabs: { key: MobileTab; label: string }[] = [
    { key: 'personal', label: t('ux.profile.tabPersonal') },
    { key: 'statutory', label: t('ux.profile.tabStatutory') },
    { key: 'security', label: t('ux.profile.tabSecurity') },
  ];

  return (
    <>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div className="page-header">
          <p className="page-subtitle">{t('portal.profile.subtitle')}</p>
          <h1 className="page-title">{profile.full_name}</h1>
        </div>
        <button
          type="button"
          onClick={() => setRevealed((prev) => !prev)}
          aria-pressed={revealed}
          className="btn-secondary !py-2 !min-h-9 self-start sm:self-auto text-xs"
        >
          {revealed ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
          {revealed ? t('ux.profile.hideSensitive') : t('ux.profile.showSensitive')}
        </button>
      </div>

      <div className="p-3 bg-amber-50 dark:bg-amber-500/10 text-amber-700 dark:text-amber-300 text-sm rounded-xl border border-amber-100 dark:border-amber-500/25">
        {t('portal.profile.managedByAdmin')}
      </div>

      {/* Mobile section tabs to prevent long 8-card scroll */}
      <div className="lg:hidden flex items-center gap-1 rounded-2xl bg-white/80 dark:bg-slate-900/80 p-1 ring-1 ring-gray-200/70 dark:ring-slate-800 overflow-x-auto">
        {tabs.map((item) => {
          const active = mobileTab === item.key;
          return (
            <button
              key={item.key}
              type="button"
              onClick={() => setMobileTab(item.key)}
              aria-pressed={active}
              className={`flex-1 whitespace-nowrap rounded-xl px-3 py-2 text-xs font-semibold transition-all ${
                active
                  ? 'bg-gradient-to-r from-[var(--accent-1)] to-[var(--accent-2)] text-white shadow-xs'
                  : 'text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-slate-100'
              }`}
            >
              {item.label}
            </button>
          );
        })}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Personal Information */}
        <div className={`card ${mobileTab !== 'personal' ? 'hidden lg:block' : ''}`}>
          <SectionIcon icon={User} label={t('employees.form.personalInfo')} num={1} />
          <div className="divide-y divide-gray-100 dark:divide-slate-800">
            <Field label={t('portal.profile.fullName')} value={profile.full_name} />
            <Field label={t('employees.form.icNumber')} value={maskSensitive(profile.ic_number, revealed)} />
            <Field label={t('portal.profile.passportNumber')} value={maskSensitive(profile.passport_number, revealed)} />
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
        <div className={`space-y-6 ${mobileTab !== 'personal' ? 'hidden lg:block' : ''}`}>
          <div className="card">
            <SectionIcon icon={MapPin} label={t('portal.profile.address')} num={2} />
            <div className="divide-y divide-gray-100 dark:divide-slate-800">
              <Field label={t('employees.form.addressLine1')} value={profile.address_line1} />
              <Field label={t('employees.form.addressLine2')} value={profile.address_line2} />
              <Field label={t('employees.form.city')} value={profile.city} />
              <Field label={t('employees.form.state')} value={profile.state} />
              <Field label={t('employees.form.postcode')} value={profile.postcode} />
            </div>
          </div>

          <div className="card">
            <SectionIcon icon={Briefcase} label={t('employees.form.employmentDetails')} num={3} />
            <div className="divide-y divide-gray-100 dark:divide-slate-800">
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

        {/* Statutory + Banking + Security */}
        <div className={`space-y-6 ${mobileTab === 'personal' ? 'hidden lg:block' : ''}`}>
          <div className={`space-y-6 ${mobileTab === 'security' ? 'hidden lg:block' : ''}`}>
            <div className="card">
              <SectionIcon icon={Shield} label={t('portal.profile.statutoryDetails')} num={4} />
              <div className="divide-y divide-gray-100 dark:divide-slate-800">
                <Field label={t('portal.profile.immigrationStatus')} value={enumLabel('residency', profile.residency_status)} />
                <Field label={t('employees.fields.epfNumber')} value={maskSensitive(profile.epf_number, revealed)} />
                <Field label={t('employees.fields.epfCategory')} value={enumLabel('epfCategory', profile.epf_category)} />
                <Field label={t('portal.profile.tin')} value={maskSensitive(profile.tax_identification_number, revealed)} />
                <Field label={t('employees.fields.socsoNumber')} value={maskSensitive(profile.socso_number, revealed)} />
                <Field label={t('employees.fields.eisNumber')} value={maskSensitive(profile.eis_number, revealed)} />
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
              <div className="divide-y divide-gray-100 dark:divide-slate-800">
                <Field label={t('employees.form.bankName')} value={profile.bank_name} />
                <Field label={t('employees.form.accountNumber')} value={maskSensitive(profile.bank_account_number, revealed)} />
              </div>
            </div>
          </div>

          <div className={`space-y-6 ${mobileTab === 'statutory' ? 'hidden lg:block' : ''}`}>
            <AppInstallCard />
            <NotificationActivationCard />
            <PasskeyManagement />
            <LinkedAccounts />
            <SessionManagement />
          </div>
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

  return (
    <div className="space-y-6">
      <CheckInCard />

      {isLoading ? (
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          <div className="card space-y-3">
            <Skeleton className="h-5 w-40" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-5/6" />
            <Skeleton className="h-4 w-4/6" />
          </div>
          <div className="card space-y-3">
            <Skeleton className="h-5 w-36" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-3/4" />
          </div>
          <div className="card space-y-3">
            <Skeleton className="h-5 w-44" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-2/3" />
          </div>
        </div>
      ) : !profile ? (
        <div className="text-center py-12 text-gray-400">{t('portal.profile.notFound')}</div>
      ) : (
        <ProfileDetails profile={profile} />
      )}
    </div>
  );
}
