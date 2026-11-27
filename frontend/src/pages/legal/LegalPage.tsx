import { ArrowLeft, BookOpen, FileCheck2, Scale, ShieldCheck } from 'lucide-react';
import { Link, useLocation } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useAuth } from '@/context/AuthContext';
import { hasOnlyEmployeeRole } from '@/lib/roles';
import { BrandLogo } from '@/components/ui/BrandLogo';
import { LanguageSwitcher } from '@/components/ui/LanguageSwitcher';

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
      : '/'
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
    <div className="min-h-[100dvh] bg-[var(--background)] text-[var(--foreground)]">
      <header className="sticky top-0 z-30 border-b border-[var(--border)] bg-[var(--card)] pt-[env(safe-area-inset-top)]">
        <div className="max-w-4xl mx-auto px-4 sm:px-6 min-h-14 py-2 flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-3 min-w-0">
            <Link
              to={backHref}
              className="inline-flex items-center gap-1.5 min-h-11 text-sm font-medium text-[var(--muted-foreground)] hover:text-[var(--foreground)] rounded-lg px-1.5 -ml-1.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors shrink-0"
            >
              <ArrowLeft className="w-4 h-4" aria-hidden="true" />
              <span>{isAuthenticated ? t('legal.header.backToApp') : t('legal.header.backToLogin')}</span>
            </Link>
            <span aria-hidden="true" className="hidden md:inline text-[var(--border)]">|</span>
            <BrandLogo variant="lockup-auto" className="hidden md:block h-7 w-auto rounded-md" />
          </div>

          <div className="flex items-center justify-end shrink-0">
            <LanguageSwitcher />
          </div>
        </div>
      </header>

      <main className="max-w-3xl mx-auto px-4 sm:px-6 py-8 sm:py-12 pb-[max(2.5rem,env(safe-area-inset-bottom))]">
        <div className="mb-8">
          <div className="inline-flex items-center gap-2 rounded-full border border-[var(--border)] bg-[var(--accent-soft)] px-3 py-1 text-xs font-semibold text-[var(--accent-1)] mb-3">
            <FileCheck2 className="w-3.5 h-3.5" aria-hidden="true" />
            <span>{t('legal.header.badge')}</span>
          </div>
          <h1 className="text-2xl sm:text-3xl font-bold tracking-tight text-[var(--foreground)]">
            {pageTitle}
          </h1>
          <p className="mt-2 text-sm sm:text-base text-[var(--muted-foreground)] max-w-3xl">
            {pageSubtitle}
          </p>
          <p className="mt-2 text-xs font-medium text-[var(--muted-foreground)]">
            {t('legal.header.effectiveDate')}
          </p>
        </div>

        <nav className="flex flex-wrap gap-2 mb-8 border-b border-[var(--border)] pb-4">
          {tabs.map((tab) => {
            const Icon = tab.icon;
            const selected = activeTab === tab.id;
            return (
              <Link
                key={tab.id}
                to={tab.href}
                aria-current={selected ? 'page' : undefined}
                className={`inline-flex items-center gap-2 min-h-10 rounded-lg px-3.5 py-2 text-sm font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors ${
                  selected
                    ? 'bg-[var(--accent-1)] text-white border border-[var(--accent-1)]'
                    : 'bg-[var(--card)] text-[var(--muted-foreground)] border border-[var(--border)] hover:text-[var(--foreground)] hover:border-[var(--foreground)]/30'
                }`}
              >
                <Icon className="w-4 h-4" aria-hidden="true" />
                <span>{t(tab.labelKey)}</span>
              </Link>
            );
          })}
        </nav>

        <div className="space-y-4">
          {sections.map((section) => (
            <section
              key={section.titleKey}
              className="rounded-xl border border-[var(--border)] bg-[var(--card)] p-5 sm:p-6"
            >
              <h2 className="text-base sm:text-lg font-semibold text-[var(--foreground)] mb-2">
                {t(section.titleKey)}
              </h2>
              <p className="text-sm leading-relaxed text-[var(--muted-foreground)]">
                {t(section.bodyKey)}
              </p>
            </section>
          ))}
        </div>
      </main>
    </div>
  );
}
