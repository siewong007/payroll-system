import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { AttachmentPreview } from '@/components/ui/AttachmentPreview';
import { buildLetterPreviewSrcDoc } from '@/lib/letterPreview';

describe('SecurityHardening', () => {
  it('blocks javascript: URLs in AttachmentPreview and renders unavailable fallback', async () => {
    const { container } = render(
      <AttachmentPreview url="javascript:alert(1)" name="evil.pdf" />,
    );

    await waitFor(() => expect(screen.queryByRole('status')).not.toBeInTheDocument());
    expect(screen.getByText('evil.pdf')).toBeInTheDocument();
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

  it('embeds restrictive CSP inside letter preview iframe srcDoc', () => {
    const doc = buildLetterPreviewSrcDoc('<p>Hello</p>');
    expect(doc).toContain(
      '<meta http-equiv="Content-Security-Policy" content="default-src \'none\'; style-src \'unsafe-inline\'; img-src https: data:;">',
    );
    expect(doc).toContain('<body><p>Hello</p></body>');
  });
});
