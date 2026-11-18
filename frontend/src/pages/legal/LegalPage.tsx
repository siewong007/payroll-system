import { ArrowLeft, BookOpen, FileCheck2, Scale, ShieldCheck } from 'lucide-react';
import { Link, useLocation } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useAuth } from '@/context/AuthContext';
import { hasOnlyEmployeeRole } from '@/lib/roles';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { LanguageSwitcher } from '@/components/ui/LanguageSwitcher';
import { AppearanceSwitcher } from '@/components/ui/AppearanceSwitcher';

type LegalTab = 'terms' | 'privacy' | 'compliance';

function resolveTab(pathname: string): LegalTab {
  if (pathname.startsWith('/privacy')) return 'privacy';
  if (pathname.startsWith('/compliance')) return 'compliance';
  return 'terms';
}

export function LegalPage() {
  const { t } = useTranslation();
  const location = useLocation();
  const { isAuthenticated, user } = useAuth();
  const activeTab = resolveTab(location.pathname);

  const backHref = isAuthenticated
    ? hasOnlyEmployeeRole(user)
      ? '/portal'
      : '/dashboard'
    : '/login';

  const tabs: { id: LegalTab; href: string; icon: typeof Scale; labelKey: string }[] = [
    { id: 'terms', href: '/terms', icon: Scale, labelKey: 'legal.tabs.terms' },
    { id: 'privacy', href: '/privacy', icon: ShieldCheck, labelKey: 'legal.tabs.privacy' },
    { id: 'compliance', href: '/compliance', icon: BookOpen, labelKey: 'legal.tabs.compliance' },
  ];

  const termsSections = [
    { titleKey: 'legal.terms.s1Title', bodyKey: 'legal.terms.s1Body' },
    { titleKey: 'legal.terms.s2Title', bodyKey: 'legal.terms.s2Body' },
    { titleKey: 'legal.terms.s3Title', bodyKey: 'legal.terms.s3Body' },
    { titleKey: 'legal.terms.s4Title', bodyKey: 'legal.terms.s4Body' },
    { titleKey: 'legal.terms.s5Title', bodyKey: 'legal.terms.s5Body' },
  ];

  const privacySections = [
    { titleKey: 'legal.privacy.s1Title', bodyKey: 'legal.privacy.s1Body' },
    { titleKey: 'legal.privacy.s2Title', bodyKey: 'legal.privacy.s2Body' },
    { titleKey: 'legal.privacy.s3Title', bodyKey: 'legal.privacy.s3Body' },
    { titleKey: 'legal.privacy.s4Title', bodyKey: 'legal.privacy.s4Body' },
  ];

  const complianceSections = [
    { titleKey: 'legal.compliance.eaTitle', bodyKey: 'legal.compliance.eaBody' },
    { titleKey: 'legal.compliance.mwoTitle', bodyKey: 'legal.compliance.mwoBody' },
    { titleKey: 'legal.compliance.epfTitle', bodyKey: 'legal.compliance.epfBody' },
    { titleKey: 'legal.compliance.perkesoTitle', bodyKey: 'legal.compliance.perkesoBody' },
    { titleKey: 'legal.compliance.lhdnTitle', bodyKey: 'legal.compliance.lhdnBody' },
  ];

  const sections =
    activeTab === 'privacy'
      ? privacySections
      : activeTab === 'compliance'
        ? complianceSections
        : termsSections;

  const pageTitle =
    activeTab === 'privacy'
      ? t('legal.privacy.title')
      : activeTab === 'compliance'
        ? t('legal.compliance.title')
        : t('legal.terms.title');

  const pageSubtitle =
    activeTab === 'privacy'
      ? t('legal.privacy.subtitle')
      : activeTab === 'compliance'
        ? t('legal.compliance.subtitle')
        : t('legal.terms.subtitle');

  return (
    <div className="min-h-screen bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100">
      <header className="sticky top-0 z-30 border-b border-slate-200/80 dark:border-slate-800 bg-white/85 dark:bg-slate-900/85 backdrop-blur-xl">
        <div className="max-w-5xl mx-auto px-4 sm:px-6 h-16 flex items-center justify-between gap-4">
          <div className="flex items-center gap-4">
            <Link
              to={backHref}
              className="inline-flex items-center gap-1.5 text-sm font-medium text-slate-600 dark:text-slate-300 hover:text-slate-900 dark:hover:text-white transition-colors"
            >
              <ArrowLeft className="w-4 h-4" />
              <span>{isAuthenticated ? t('legal.header.backToApp') : t('legal.header.backToLogin')}</span>
            </Link>
            <span className="hidden sm:inline text-slate-300 dark:text-slate-700">|</span>
            <BrandLogo variant="lockup-dark" className="hidden sm:block h-7 w-auto dark:brightness-125" />
          </div>

          <div className="flex items-center gap-2">
            <AppearanceSwitcher />
            <LanguageSwitcher />
          </div>
        </div>
      </header>

      <main className="max-w-5xl mx-auto px-4 sm:px-6 py-8 sm:py-12">
        <div className="mb-8">
          <div className="inline-flex items-center gap-2 rounded-full border border-indigo-200/80 dark:border-indigo-500/30 bg-indigo-50 dark:bg-indigo-500/10 px-3 py-1 text-xs font-semibold text-indigo-700 dark:text-indigo-300 mb-3">
            <FileCheck2 className="w-3.5 h-3.5" />
            <span>{t('legal.header.badge')}</span>
          </div>
          <h1 className="text-2xl sm:text-3xl font-bold tracking-tight text-slate-900 dark:text-white">
            {pageTitle}
          </h1>
          <p className="mt-2 text-sm sm:text-base text-slate-600 dark:text-slate-400 max-w-3xl">
            {pageSubtitle}
          </p>
          <p className="mt-2 text-xs font-medium text-slate-500 dark:text-slate-500">
            {t('legal.header.effectiveDate')}
          </p>
        </div>

        <nav className="flex flex-wrap gap-2 mb-8 border-b border-slate-200 dark:border-slate-800 pb-4">
          {tabs.map((tab) => {
            const Icon = tab.icon;
            const selected = activeTab === tab.id;
            return (
              <Link
                key={tab.id}
                to={tab.href}
                className={`inline-flex items-center gap-2 rounded-xl px-4 py-2.5 text-sm font-medium transition-all ${
                  selected
                    ? 'bg-slate-900 text-white dark:bg-indigo-500 dark:text-white shadow-sm'
                    : 'bg-white dark:bg-slate-900 text-slate-600 dark:text-slate-300 border border-slate-200 dark:border-slate-800 hover:border-slate-300 dark:hover:border-slate-700'
                }`}
              >
                <Icon className="w-4 h-4" />
                <span>{t(tab.labelKey)}</span>
              </Link>
            );
          })}
        </nav>

        <div className="space-y-4">
          {sections.map((section) => (
            <section
              key={section.titleKey}
              className="rounded-2xl border border-slate-200/80 dark:border-slate-800 bg-white dark:bg-slate-900/70 p-5 sm:p-6 shadow-xs"
            >
              <h2 className="text-base sm:text-lg font-semibold text-slate-900 dark:text-white mb-2">
                {t(section.titleKey)}
              </h2>
              <p className="text-sm leading-relaxed text-slate-600 dark:text-slate-300">
                {t(section.bodyKey)}
              </p>
            </section>
          ))}
        </div>
      </main>
    </div>
  );
}
