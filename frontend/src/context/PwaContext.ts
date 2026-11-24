import { createContext, useContext } from 'react';
import type { NotificationStatus, PwaPlatform } from '@/lib/pwa';

export type InstallOutcome = 'accepted' | 'dismissed' | 'guide' | 'installed';

export interface PwaContextValue {
  platform: PwaPlatform;
  isIos: boolean;
  isStandalone: boolean;
  canPromptInstall: boolean;
  canShowInstallGuide: boolean;
  bannerDismissed: boolean;
  installModalOpen: boolean;
  notificationStatus: NotificationStatus;
  notificationBusy: boolean;
  swReady: boolean;
  openInstallModal: () => void;
  closeInstallModal: () => void;
  dismissBanner: () => void;
  triggerInstall: () => Promise<InstallOutcome>;
  enableNotifications: () => Promise<NotificationStatus>;
  sendTestNotification: () => Promise<boolean>;
}

const noopAsyncInstall = async (): Promise<InstallOutcome> => 'guide';
const noopAsyncNotify = async (): Promise<NotificationStatus> => 'unsupported';
const noopAsyncTest = async (): Promise<boolean> => false;

const fallbackContext: PwaContextValue = {
  platform: 'desktop',
  isIos: false,
  isStandalone: false,
  canPromptInstall: false,
  canShowInstallGuide: false,
  bannerDismissed: true,
  installModalOpen: false,
  notificationStatus: 'unsupported',
  notificationBusy: false,
  swReady: false,
  openInstallModal: () => undefined,
  closeInstallModal: () => undefined,
  dismissBanner: () => undefined,
  triggerInstall: noopAsyncInstall,
  enableNotifications: noopAsyncNotify,
  sendTestNotification: noopAsyncTest,
};

export const PwaContext = createContext<PwaContextValue>(fallbackContext);

export function usePwa(): PwaContextValue {
  return useContext(PwaContext);
}
