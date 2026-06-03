/**
 * Modal.a11y.test.jsx — Axe accessibility regression tests for the shared
 * Modal component (src/components/common/Modal.jsx).
 *
 * The real Modal renders into a portal on document.body and uses an open/close
 * animation effect (requestAnimationFrame + setTimeout), so we render the REAL
 * component, wait for it to mount, and run axe against the live document body.
 *
 * IMPORTANT: every axe() call is awaited and tests never run concurrently
 * (see vitest.a11y.config.js → fileParallelism:false, sequence.concurrent:false)
 * so two axe runs are never in flight in the shared jsdom instance at once.
 *
 * NOTE: color-contrast is disabled because jsdom cannot compute CSS values.
 */

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { axe, toHaveNoViolations } from 'vitest-axe';

import Modal from './Modal';

expect.extend(toHaveNoViolations);

const AXE_OPTIONS = {
  rules: {
    'color-contrast': { enabled: false },
  },
};

/**
 * Renders a Modal and resolves once its dialog has appeared in the portal.
 * Returns document.body so axe scans the portalled content too.
 */
async function renderOpenModal(props = {}, body = <p>Modal body content</p>) {
  render(
    <Modal isOpen onClose={() => {}} title="Confirm action" {...props}>
      {body}
    </Modal>
  );
  // Wait for the portalled dialog to be present before scanning.
  await screen.findByRole('dialog');
  return document.body;
}

describe('Modal component — axe accessibility', () => {
  it('open dialog with title has no serious/critical violations', async () => {
    const container = await renderOpenModal();
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('dialog exposes role=dialog, aria-modal and an accessible name', async () => {
    await renderOpenModal({ title: 'Delete item?' });
    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    // aria-labelledby points at the rendered title heading.
    expect(dialog).toHaveAttribute('aria-labelledby');
    expect(screen.getByRole('heading', { name: /delete item\?/i })).toBeInTheDocument();
  });

  it('close button has an accessible name (button-name rule)', async () => {
    const container = await renderOpenModal({ showClose: true });
    const results = await axe(container, {
      ...AXE_OPTIONS,
      runOnly: { type: 'rule', values: ['button-name', 'aria-allowed-attr', 'aria-required-attr'] },
    });
    expect(results).toHaveNoViolations();
  });

  it('titled modal without a close button still has no violations', async () => {
    // A dialog must have an accessible name, so we keep a title (which provides
    // aria-labelledby) while exercising the no-close-button code path and a
    // labelled form inside the modal body.
    const container = await renderOpenModal(
      { title: 'Quick form', showClose: false },
      <form>
        <label htmlFor="m-name">Name</label>
        <input id="m-name" type="text" />
      </form>
    );
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });

  it('fullscreen size variant has no violations', async () => {
    const container = await renderOpenModal({ size: 'fullscreen', title: 'Editor' });
    const results = await axe(container, AXE_OPTIONS);
    expect(results).toHaveNoViolations();
  });
});
