import { describe, it, expect } from 'vitest';
import { parseUserAgent, deviceLabel } from '@/lib/userAgent';
import { formatRelativeTime } from '@/lib/utils';

const CHROME_MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36';
const SAFARI_IPHONE = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1';
const EDGE_WIN = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0';
const IPAD = 'Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/604.1';

describe('parseUserAgent', () => {
  it('parses Chrome on macOS as desktop', () => {
    expect(parseUserAgent(CHROME_MAC)).toEqual({ deviceType: 'desktop', os: 'macOS', browser: 'Chrome' });
  });
  it('parses Safari on iPhone as mobile', () => {
    expect(parseUserAgent(SAFARI_IPHONE)).toEqual({ deviceType: 'mobile', os: 'iOS', browser: 'Safari' });
  });
  it('detects Edge before Chrome (its UA contains Chrome)', () => {
    expect(parseUserAgent(EDGE_WIN).browser).toBe('Edge');
  });
  it('detects iPad as tablet', () => {
    expect(parseUserAgent(IPAD)).toEqual({ deviceType: 'tablet', os: 'iOS', browser: 'Safari' });
  });
  it('returns unknowns for missing/garbage UA', () => {
    expect(parseUserAgent(null).deviceType).toBe('unknown');
    expect(deviceLabel(parseUserAgent(null))).toBe('Unknown device');
    expect(deviceLabel(parseUserAgent('curl/8.0'))).toBe('Unknown device');
  });
  it('labels a parsed device as "Browser on OS"', () => {
    expect(deviceLabel(parseUserAgent(CHROME_MAC))).toBe('Chrome on macOS');
  });
});

describe('formatRelativeTime', () => {
  it('formats recency buckets', () => {
    const now = Date.now();
    expect(formatRelativeTime(new Date(now - 30_000))).toBe('just now');
    expect(formatRelativeTime(new Date(now - 5 * 60_000))).toBe('5 min ago');
    expect(formatRelativeTime(new Date(now - 3 * 3_600_000))).toBe('3 hr ago');
    expect(formatRelativeTime(new Date(now - 2 * 86_400_000))).toBe('2 days ago');
  });
});
