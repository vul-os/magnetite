// ReplayScrubber.test.jsx — AGENT 5 tests for the Replay scrubber/controls component.
//
// ReplayScrubber is a pure-UI component: given props it renders controls
// and fires callbacks.  No API calls; no async.
//
// Coverage:
//   1. Rendering in paused state (Play button visible)
//   2. Rendering in playing state (Pause button visible)
//   3. Tick counter display
//   4. Progress range input value
//   5. Speed buttons render and mark active speed
//   6. onPlay callback fires on Play/Pause click
//   7. onSeek fires with tick=0 on Rewind
//   8. onSeek fires with totalTicks on Skip-to-end
//   9. onSeek fires computed tick from range input change
//  10. onSpeedChange fires with the selected speed
//  11. aria-label and role attributes
//  12. aria-valuenow / aria-valuemax on range input

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import ReplayScrubber from './ReplayScrubber';

// ── Mock CSS ──────────────────────────────────────────────────────────────────

vi.mock('./ReplayScrubber.css', () => ({}));

// ── Default props ─────────────────────────────────────────────────────────────

function defaultProps(overrides = {}) {
  return {
    currentTick: 0,
    totalTicks: 100,
    playing: false,
    speed: 1,
    onPlay: vi.fn(),
    onSeek: vi.fn(),
    onSpeedChange: vi.fn(),
    ...overrides,
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ReplayScrubber — paused state', () => {
  it('shows a Play button when not playing', () => {
    render(<ReplayScrubber {...defaultProps({ playing: false })} />);
    expect(screen.getByRole('button', { name: /play/i })).toBeInTheDocument();
  });

  it('does NOT show a Pause button when not playing', () => {
    render(<ReplayScrubber {...defaultProps({ playing: false })} />);
    expect(screen.queryByRole('button', { name: /pause/i })).not.toBeInTheDocument();
  });
});

describe('ReplayScrubber — playing state', () => {
  it('shows a Pause button when playing', () => {
    render(<ReplayScrubber {...defaultProps({ playing: true })} />);
    expect(screen.getByRole('button', { name: /pause/i })).toBeInTheDocument();
  });

  it('does NOT show a Play button when playing', () => {
    render(<ReplayScrubber {...defaultProps({ playing: true })} />);
    expect(screen.queryByRole('button', { name: /^play$/i })).not.toBeInTheDocument();
  });
});

describe('ReplayScrubber — tick counter', () => {
  it('shows "currentTick / totalTicks" in the tick counter', () => {
    render(<ReplayScrubber {...defaultProps({ currentTick: 42, totalTicks: 300 })} />);
    expect(screen.getByText(/42/)).toBeInTheDocument();
    expect(screen.getByText(/300/)).toBeInTheDocument();
  });

  it('tick counter uses aria-live polite', () => {
    render(<ReplayScrubber {...defaultProps()} />);
    const counter = document.querySelector('[aria-live="polite"]');
    expect(counter).not.toBeNull();
  });
});

describe('ReplayScrubber — range input', () => {
  it('range input value equals (currentTick / totalTicks) * 100', () => {
    render(<ReplayScrubber {...defaultProps({ currentTick: 50, totalTicks: 100 })} />);
    const range = screen.getByRole('slider');
    // 50/100 * 100 = 50
    expect(Number(range.value)).toBe(50);
  });

  it('range input value is 0 when at start', () => {
    render(<ReplayScrubber {...defaultProps({ currentTick: 0, totalTicks: 100 })} />);
    const range = screen.getByRole('slider');
    expect(Number(range.value)).toBe(0);
  });

  it('range input value is 100 at end', () => {
    render(<ReplayScrubber {...defaultProps({ currentTick: 100, totalTicks: 100 })} />);
    const range = screen.getByRole('slider');
    expect(Number(range.value)).toBe(100);
  });

  it('aria-valuenow reflects currentTick', () => {
    render(<ReplayScrubber {...defaultProps({ currentTick: 77, totalTicks: 200 })} />);
    const range = screen.getByRole('slider');
    expect(range.getAttribute('aria-valuenow')).toBe('77');
  });

  it('aria-valuemax reflects totalTicks', () => {
    render(<ReplayScrubber {...defaultProps({ currentTick: 0, totalTicks: 250 })} />);
    const range = screen.getByRole('slider');
    expect(range.getAttribute('aria-valuemax')).toBe('250');
  });

  it('aria-valuetext mentions currentTick and totalTicks', () => {
    render(<ReplayScrubber {...defaultProps({ currentTick: 10, totalTicks: 50 })} />);
    const range = screen.getByRole('slider');
    const text = range.getAttribute('aria-valuetext') || '';
    expect(text).toMatch(/10/);
    expect(text).toMatch(/50/);
  });
});

describe('ReplayScrubber — speed buttons', () => {
  const SPEEDS = [0.25, 0.5, 1, 2, 4];

  it('renders buttons for all five speeds', () => {
    render(<ReplayScrubber {...defaultProps()} />);
    for (const s of SPEEDS) {
      expect(screen.getByRole('button', { name: `${s}× speed` })).toBeInTheDocument();
    }
  });

  it('marks active speed button with aria-pressed=true', () => {
    render(<ReplayScrubber {...defaultProps({ speed: 2 })} />);
    const btn2x = screen.getByRole('button', { name: '2× speed' });
    expect(btn2x.getAttribute('aria-pressed')).toBe('true');
  });

  it('marks inactive speed buttons with aria-pressed=false', () => {
    render(<ReplayScrubber {...defaultProps({ speed: 1 })} />);
    const btn025 = screen.getByRole('button', { name: '0.25× speed' });
    expect(btn025.getAttribute('aria-pressed')).toBe('false');
  });
});

describe('ReplayScrubber — callbacks', () => {
  it('calls onPlay when Play/Pause button is clicked', () => {
    const onPlay = vi.fn();
    render(<ReplayScrubber {...defaultProps({ onPlay })} />);
    fireEvent.click(screen.getByRole('button', { name: /play/i }));
    expect(onPlay).toHaveBeenCalledTimes(1);
  });

  it('calls onSeek(0) when Rewind button is clicked', () => {
    const onSeek = vi.fn();
    render(<ReplayScrubber {...defaultProps({ onSeek })} />);
    fireEvent.click(screen.getByRole('button', { name: /rewind/i }));
    expect(onSeek).toHaveBeenCalledWith(0);
  });

  it('calls onSeek(totalTicks) when Skip-to-end button is clicked', () => {
    const onSeek = vi.fn();
    render(<ReplayScrubber {...defaultProps({ totalTicks: 150, onSeek })} />);
    fireEvent.click(screen.getByRole('button', { name: /skip to end/i }));
    expect(onSeek).toHaveBeenCalledWith(150);
  });

  it('calls onSeek with computed tick when range input changes', () => {
    const onSeek = vi.fn();
    render(<ReplayScrubber {...defaultProps({ totalTicks: 200, onSeek })} />);
    const range = screen.getByRole('slider');
    // Setting value to 50 → tick = round((50/100) * 200) = 100
    fireEvent.change(range, { target: { value: '50' } });
    expect(onSeek).toHaveBeenCalledWith(100);
  });

  it('calls onSeek with tick=0 when range is set to 0', () => {
    const onSeek = vi.fn();
    render(<ReplayScrubber {...defaultProps({ currentTick: 50, totalTicks: 100, onSeek })} />);
    const range = screen.getByRole('slider');
    fireEvent.change(range, { target: { value: '0' } });
    expect(onSeek).toHaveBeenCalledWith(0);
  });

  it('calls onSpeedChange with the selected speed value', () => {
    const onSpeedChange = vi.fn();
    render(<ReplayScrubber {...defaultProps({ onSpeedChange })} />);
    fireEvent.click(screen.getByRole('button', { name: '4× speed' }));
    expect(onSpeedChange).toHaveBeenCalledWith(4);
  });

  it('calls onSpeedChange(0.25) when 0.25× button is clicked', () => {
    const onSpeedChange = vi.fn();
    render(<ReplayScrubber {...defaultProps({ onSpeedChange })} />);
    fireEvent.click(screen.getByRole('button', { name: '0.25× speed' }));
    expect(onSpeedChange).toHaveBeenCalledWith(0.25);
  });

  it('does not throw when callbacks are omitted', () => {
    // onPlay / onSeek / onSpeedChange are optional
    render(
      <ReplayScrubber
        currentTick={0}
        totalTicks={100}
        playing={false}
        speed={1}
        // no callbacks
      />
    );
    // clicking should be safe
    const playBtn = screen.getByRole('button', { name: /play/i });
    expect(() => fireEvent.click(playBtn)).not.toThrow();
  });
});

describe('ReplayScrubber — accessibility', () => {
  it('has a group role with aria-label for the root', () => {
    render(<ReplayScrubber {...defaultProps()} />);
    expect(screen.getByRole('group', { name: /replay playback controls/i })).toBeInTheDocument();
  });

  it('has a group role with aria-label for the speed section', () => {
    render(<ReplayScrubber {...defaultProps()} />);
    expect(screen.getByRole('group', { name: /playback speed/i })).toBeInTheDocument();
  });

  it('range input has aria-label "Replay position"', () => {
    render(<ReplayScrubber {...defaultProps()} />);
    expect(screen.getByLabelText(/replay position/i)).toBeInTheDocument();
  });
});

describe('ReplayScrubber — edge cases', () => {
  it('handles totalTicks=0 without division errors', () => {
    render(<ReplayScrubber {...defaultProps({ currentTick: 0, totalTicks: 0 })} />);
    const range = screen.getByRole('slider');
    expect(Number(range.value)).toBe(0);
  });

  it('clamps pct to 100 if currentTick > totalTicks', () => {
    render(<ReplayScrubber {...defaultProps({ currentTick: 200, totalTicks: 100 })} />);
    const range = screen.getByRole('slider');
    expect(Number(range.value)).toBeLessThanOrEqual(100);
  });
});
