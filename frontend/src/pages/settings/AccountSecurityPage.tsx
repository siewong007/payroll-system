import { useTranslation } from 'react-i18next';
import { PageHeader } from '@/components/ui/PageHeader';
import { ChangePasswordCard } from '@/components/ChangePasswordCard';
import { TwoFactorSetup } from '@/components/TwoFactorSetup';
import { PasskeyManagement } from '@/components/PasskeyManagement';
import { LinkedAccounts } from '@/components/LinkedAccounts';
import { SessionManagement } from '@/components/SessionManagement';
import { AppInstallCard } from '@/components/pwa/AppInstallCard';
import { NotificationActivationCard } from '@/components/pwa/NotificationActivationCard';

/**
 * Self-service security, sessions, and device preferences for every user inside
 * `AppLayout` (including `super_admin`, `hr_manager`, `payroll_admin`, `finance`,
 * and `exec`, who cannot access company-level `/settings`).
 */
export function AccountSecurityPage() {
  const { t } = useTranslation();

  return (
    <div className="space-y-8">
      <PageHeader
        title={t('settings.accountPage.title')}
        subtitle={t('settings.accountPage.subtitle')}
      />

      <section aria-labelledby="account-app-heading" className="space-y-3">
        <h2
          id="account-app-heading"
          className="text-xs font-semibold uppercase tracking-widest text-gray-600 dark:text-slate-400"
        >
          {t('settings.sections.app.label')}
        </h2>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 items-start">
          <AppInstallCard />
          <NotificationActivationCard />
        </div>
      </section>

      <section aria-labelledby="account-security-heading" className="space-y-3">
        <h2
          id="account-security-heading"
          className="text-xs font-semibold uppercase tracking-widest text-gray-600 dark:text-slate-400"
        >
          {t('settings.sections.security.label')}
        </h2>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 items-start">
          <div className="space-y-6">
            <ChangePasswordCard />
            <LinkedAccounts />
          </div>
          <div className="space-y-6">
            <TwoFactorSetup />
            <PasskeyManagement />
          </div>
        </div>
      </section>

      <section aria-labelledby="account-sessions-heading" className="space-y-3">
        <h2
          id="account-sessions-heading"
          className="text-xs font-semibold uppercase tracking-widest text-gray-600 dark:text-slate-400"
        >
          {t('settings.sections.sessions.label')}
        </h2>
        <SessionManagement />
      </section>
    </div>
  );
}
