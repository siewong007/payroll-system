export type DeviceType = 'desktop' | 'mobile' | 'tablet' | 'unknown';

export interface ParsedUserAgent {
  deviceType: DeviceType;
  os: string;
  browser: string;
}

/** Order matters: Edge/Opera UAs also contain "Chrome", and Chrome's contains "Safari". */
export function parseUserAgent(ua: string | null | undefined): ParsedUserAgent {
  if (!ua) return { deviceType: 'unknown', os: 'Unknown OS', browser: 'Unknown browser' };

  const browser = /Edg(e|A|iOS)?\//.test(ua) ? 'Edge'
    : /OPR\/|Opera/.test(ua) ? 'Opera'
    : /Chrome\//.test(ua) ? 'Chrome'
    : /Firefox\//.test(ua) ? 'Firefox'
    : /Safari\//.test(ua) ? 'Safari'
    : 'Unknown browser';

  const os = /iPhone|iPad|iPod/.test(ua) ? 'iOS'
    : /Android/.test(ua) ? 'Android'
    : /Windows/.test(ua) ? 'Windows'
    : /Mac OS X|Macintosh/.test(ua) ? 'macOS'
    : /CrOS/.test(ua) ? 'ChromeOS'
    : /Linux/.test(ua) ? 'Linux'
    : 'Unknown OS';

  const deviceType: DeviceType =
    /iPad|Tablet/.test(ua) || (/Android/.test(ua) && !/Mobile/.test(ua)) ? 'tablet'
    : /iPhone|iPod|Mobile/.test(ua) ? 'mobile'
    : os === 'Unknown OS' && browser === 'Unknown browser' ? 'unknown'
    : 'desktop';

  return { deviceType, os, browser };
}

export function deviceLabel(p: ParsedUserAgent): string {
  if (p.deviceType === 'unknown' && p.browser === 'Unknown browser') return 'Unknown device';
  if (p.browser === 'Unknown browser') return `Browser on ${p.os}`;
  return `${p.browser} on ${p.os}`;
}
