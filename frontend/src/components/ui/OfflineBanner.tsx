import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { WifiOff } from 'lucide-react';

export function OfflineBanner() {
  const { t } = useTranslation();
  const [isOffline, setIsOffline] = useState(
    typeof navigator !== 'undefined' ? !navigator.onLine : false,
  );

  useEffect(() => {
    const handleOnline = () => setIsOffline(false);
    const handleOffline = () => setIsOffline(true);

    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);
    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, []);

  if (!isOffline) {
    return null;
  }

  return (
    <div
      role="status"
      aria-live="polite"
      className="sticky top-0 z-[70] flex items-center justify-center gap-2 border-b border-amber-300 bg-amber-50 px-4 py-2 text-xs font-medium text-amber-900 dark:border-amber-500/40 dark:bg-amber-950/90 dark:text-amber-100"
    >
      <WifiOff className="h-4 w-4 shrink-0" aria-hidden="true" />
      <span>{t('common.offlineBanner')}</span>
    </div>
  );
}
