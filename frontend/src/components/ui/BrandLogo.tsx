type BrandLogoVariant = 'icon-dark' | 'icon-primary' | 'lockup-dark' | 'lockup-light' | 'lockup-auto';

interface BrandLogoProps {
  variant?: BrandLogoVariant;
  alt?: string;
  className?: string;
  priority?: boolean;
}

const BRAND_ASSETS: Record<Exclude<BrandLogoVariant, 'lockup-auto'>, { src: string; width: number; height: number }> = {
  'icon-dark': { src: '/branding/payrollmy-icon-dark.svg', width: 72, height: 72 },
  'icon-primary': { src: '/branding/payrollmy-icon-primary.svg', width: 72, height: 72 },
  'lockup-dark': { src: '/branding/payrollmy-lockup-dark.svg', width: 320, height: 72 },
  'lockup-light': { src: '/branding/payrollmy-lockup-light.svg', width: 320, height: 72 },
};

export function BrandLogo({
  variant = 'lockup-dark',
  alt = 'PayrollMY',
  className,
  priority = false,
}: BrandLogoProps) {
  if (variant === 'lockup-auto') {
    const light = BRAND_ASSETS['lockup-light'];
    const dark = BRAND_ASSETS['lockup-dark'];
    return (
      <>
        <img
          src={light.src}
          alt={alt}
          width={light.width}
          height={light.height}
          decoding="async"
          fetchPriority={priority ? 'high' : undefined}
          className={`dark:hidden ${className ?? ''}`}
          draggable={false}
        />
        <img
          src={dark.src}
          alt={alt}
          width={dark.width}
          height={dark.height}
          decoding="async"
          fetchPriority={priority ? 'high' : undefined}
          className={`hidden dark:block rounded-md ${className ?? ''}`}
          draggable={false}
        />
      </>
    );
  }

  const asset = BRAND_ASSETS[variant];
  return (
    <img
      src={asset.src}
      alt={alt}
      width={asset.width}
      height={asset.height}
      decoding="async"
      fetchPriority={priority ? 'high' : undefined}
      className={className}
      draggable={false}
    />
  );
}
