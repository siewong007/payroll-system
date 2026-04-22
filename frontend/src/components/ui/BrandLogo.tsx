type BrandLogoVariant = 'icon-dark' | 'icon-primary' | 'lockup-dark' | 'lockup-light';

interface BrandLogoProps {
  variant?: BrandLogoVariant;
  alt?: string;
  className?: string;
}

const BRAND_ASSETS: Record<BrandLogoVariant, string> = {
  'icon-dark': '/branding/payrollmy-icon-dark.svg',
  'icon-primary': '/branding/payrollmy-icon-primary.svg',
  'lockup-dark': '/branding/payrollmy-lockup-dark.svg',
  'lockup-light': '/branding/payrollmy-lockup-light.svg',
};

export function BrandLogo({
  variant = 'lockup-dark',
  alt = 'PayrollMY',
  className,
}: BrandLogoProps) {
  return (
    <img
      src={BRAND_ASSETS[variant]}
      alt={alt}
      className={className}
      draggable={false}
    />
  );
}
