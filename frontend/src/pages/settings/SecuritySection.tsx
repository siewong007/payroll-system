import { ChangePasswordCard } from '@/components/ChangePasswordCard';
import { TwoFactorSetup } from '@/components/TwoFactorSetup';
import { PasskeyManagement } from '@/components/PasskeyManagement';
import { LinkedAccounts } from '@/components/LinkedAccounts';

export function SecuritySection() {
  return (
    <div className="space-y-6">
      <ChangePasswordCard />
      <TwoFactorSetup />
      <PasskeyManagement />
      <LinkedAccounts />
    </div>
  );
}
