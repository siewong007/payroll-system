import { useTranslation } from 'react-i18next';
import { PageHeader } from '@/components/ui/PageHeader';
import { SecuritySection } from './SecuritySection';
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
    <div className="max-w-4xl space-y-6">
      <PageHeader
        title={t('settings.accountPage.title')}
        subtitle={t('settings.accountPage.subtitle')}
      />
      <SecuritySection />
      <SessionManagement />
      <div className="space-y-6">
        <AppInstallCard />
        <NotificationActivationCard />
      </div>
    </div>
  );
}
