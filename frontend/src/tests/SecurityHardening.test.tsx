import fs from 'node:fs';
import path from 'node:path';
import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { AttachmentLink, AttachmentPreview } from '@/components/ui/AttachmentPreview';
import { buildLetterPreviewSrcDoc } from '@/lib/letterPreview';

describe('SecurityHardening', () => {
  it('blocks javascript: and insecure http: URLs in AttachmentPreview and AttachmentLink', async () => {
    const { container } = render(
      <>
        <AttachmentPreview url="javascript:alert(1)" name="evil.pdf" />
        <AttachmentPreview url="http://example.com/plain.pdf" name="plain.pdf" />
        <AttachmentLink url="http://example.com/plain.pdf">Insecure link</AttachmentLink>
      </>,
    );

    await waitFor(() => expect(screen.queryByRole('status')).not.toBeInTheDocument());
    expect(screen.getByText('evil.pdf')).toBeInTheDocument();
    expect(screen.getAllByText('plain.pdf').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('Insecure link').tagName).toBe('SPAN');
    expect(container.querySelector('a')).toBeNull();
    expect(container.querySelector('img')).toBeNull();
  });

  it('allows https: URLs in AttachmentPreview with rel="noopener noreferrer"', async () => {
    render(
      <AttachmentPreview url="https://example.com/receipt.pdf" name="receipt.pdf" />,
    );

    const link = await screen.findByRole('link');
    expect(link).toHaveAttribute('href', 'https://example.com/receipt.pdf');
    expect(link).toHaveAttribute('rel', 'noopener noreferrer');
  });

  it('embeds hash-pinned style CSP and blocks remote images inside letter preview iframe srcDoc', () => {
    const doc = buildLetterPreviewSrcDoc('<p>Hello</p>');
    expect(doc).toContain(
      '<meta http-equiv="Content-Security-Policy" content="default-src \'none\'; style-src \'sha256-mZLnuTGMILdxsYZY6gYcvHgeG0Q9cITqybisGfV9bR0=\'; img-src \'none\';">',
    );
    expect(doc).not.toContain('unsafe-inline');
    expect(doc).toContain('<body><p>Hello</p></body>');
  });

  it('ships hash-pinned style CSP in offline.html', () => {
    const offlinePath = path.resolve(import.meta.dirname, '../../public/offline.html');
    const html = fs.readFileSync(offlinePath, 'utf8');
    expect(html).toContain(
      "default-src 'none'; img-src 'self'; style-src 'sha256-2ne3uzlLXcFuD/fcSItZ2q3fjzFQwMjCGb7SbnBPSS0=';",
    );
  });
});
