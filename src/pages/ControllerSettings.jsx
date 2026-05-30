import { useState } from 'react';
import Layout from '../components/Layout';
import { useGamepad } from '../hooks/useGamepad';
import './ControllerSettings.css';

// ── Icons ─────────────────────────────────────────────────────────────────────

function GamepadIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <line x1="6" y1="12" x2="10" y2="12" />
      <line x1="8"  y1="10" x2="8"  y2="14" />
      <line x1="15" y1="13" x2="15" y2="13" strokeWidth="2" strokeLinecap="round" />
      <line x1="18" y1="11" x2="18" y2="11" strokeWidth="2" strokeLinecap="round" />
      <path d="M6 5a2 2 0 0 0-2 2l-.643 9.646A2 2 0 0 0 5.35 19h13.3a2 2 0 0 0 1.993-2.354L20 7a2 2 0 0 0-2-2H6z" />
    </svg>
  );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const ACTION_LABELS = {
  move_forward:   'Move Forward',
  move_backward:  'Move Backward',
  move_left:      'Move Left',
  move_right:     'Move Right',
  aim_horizontal: 'Aim Horizontal',
  aim_vertical:   'Aim Vertical',
  fire:           'Fire / Shoot',
  aim:            'Aim Down Sights',
  jump:           'Jump',
  interact:       'Interact / Use',
  reload:         'Reload',
  sprint:         'Sprint (L3)',
  map:            'Open Map',
  pause:          'Pause / Menu',
};

const STANDARD_BUTTON_NAMES = [
  'Cross/A', 'Circle/B', 'Square/X', 'Triangle/Y',
  'L1', 'R1', 'L2', 'R2',
  'Share', 'Options', 'L3', 'R3',
  'D-Up', 'D-Down', 'D-Left', 'D-Right',
  'Guide',
];

const AXIS_NAMES = ['LX', 'LY', 'RX', 'RY', 'L2', 'R2'];

function bindingLabel(binding) {
  if (!binding) return '—';
  if (binding.type === 'button') {
    return STANDARD_BUTTON_NAMES[binding.index] ?? `Btn ${binding.index}`;
  }
  const name = AXIS_NAMES[binding.index] ?? `Axis ${binding.index}`;
  return `${name}${binding.invert ? ' (−)' : ' (+)'}`;
}

function AxisBar({ value }) {
  const pct = ((value + 1) / 2) * 100;
  return (
    <div className="axis-bar" role="meter" aria-valuenow={Math.round(value * 100)} aria-valuemin={-100} aria-valuemax={100}>
      <div className="axis-bar-fill" style={{ width: `${pct}%` }} />
      <div className="axis-bar-center" aria-hidden="true" />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────

const TABS = ['Gamepads', 'Button Test', 'Input Bindings'];

export default function ControllerSettings() {
  const [tab, setTab] = useState('Gamepads');
  const {
    gamepads, activeGamepad, setActiveGamepad,
    axes, buttons,
    bindings, listening, startListening, cancelListening,
    clearBinding, resetBindings,
    isSupported,
  } = useGamepad();

  return (
    <Layout>
      <div className="controller-page reveal">

        {/* ── Header ── */}
        <header className="controller-header reveal-1">
          <span className="kicker">// Input System</span>
          <h1>Controller Settings</h1>
          <p className="controller-subtitle">
            Detect connected gamepads, test inputs, and remap actions to your preference.
          </p>
        </header>

        {/* ── API support notice ── */}
        {!isSupported && (
          <div className="controller-notice" role="alert">
            <strong>Gamepad API not supported</strong> in this browser. Try Chrome, Edge, or Firefox.
          </div>
        )}

        {/* ── Tab bar ── */}
        <nav className="controller-tabs reveal-2" aria-label="Controller sections">
          {TABS.map(t => (
            <button
              key={t}
              className={`ctrl-tab${tab === t ? ' active' : ''}`}
              onClick={() => setTab(t)}
              aria-current={tab === t ? 'page' : undefined}
            >
              {t}
            </button>
          ))}
        </nav>

        {/* ─── TAB: Gamepads ─── */}
        {tab === 'Gamepads' && (
          <section className="controller-section reveal-3" aria-label="Connected gamepads">
            {gamepads.length === 0 ? (
              <div className="controller-no-gamepad">
                <span className="ctrl-no-gp-icon" aria-hidden="true"><GamepadIcon /></span>
                <h3>No gamepads connected</h3>
                <p>Connect a gamepad and press any button to activate it. Supported: Xbox, PlayStation, generic USB/Bluetooth controllers.</p>
              </div>
            ) : (
              <ul className="gamepad-list" role="list">
                {gamepads.map(gp => (
                  <li key={gp.index} role="listitem">
                    <button
                      className={`gamepad-card${activeGamepad === gp.index ? ' active' : ''}`}
                      onClick={() => setActiveGamepad(gp.index)}
                    >
                      <div className="gp-icon" aria-hidden="true"><GamepadIcon /></div>
                      <div className="gp-info">
                        <span className="gp-name">{gp.id || `Gamepad ${gp.index + 1}`}</span>
                        <span className="gp-meta">{gp.buttonCount} buttons · {gp.axisCount} axes</span>
                        <span className={`gp-status ${gp.connected ? 'connected' : 'disconnected'}`}>
                          {gp.connected ? 'Connected' : 'Disconnected'}
                        </span>
                      </div>
                      {activeGamepad === gp.index && (
                        <span className="gp-active-badge" aria-label="Active controller">Active</span>
                      )}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>
        )}

        {/* ─── TAB: Button Test ─── */}
        {tab === 'Button Test' && (
          <section className="controller-section reveal-3" aria-label="Button and axis tester">
            {gamepads.length === 0 ? (
              <div className="controller-no-gamepad">
                <p>Connect a gamepad to test inputs.</p>
              </div>
            ) : (
              <>
                <h3 className="section-sub-heading">
                  Testing: <span className="gp-name-inline">{gamepads[activeGamepad]?.id || `Gamepad ${activeGamepad + 1}`}</span>
                </h3>

                {/* Buttons */}
                <div className="test-section">
                  <h4 className="test-section-label">
                    <span className="kicker">// Buttons ({buttons.length})</span>
                  </h4>
                  <div className="buttons-grid" role="group" aria-label="Buttons">
                    {buttons.map((btn, i) => (
                      <div
                        key={i}
                        className={`btn-indicator${btn.pressed ? ' pressed' : ''}${btn.touched ? ' touched' : ''}`}
                        role="status"
                        aria-label={`${STANDARD_BUTTON_NAMES[i] ?? `Button ${i}`}: ${btn.pressed ? 'pressed' : 'not pressed'}`}
                        title={STANDARD_BUTTON_NAMES[i] ?? `Button ${i}`}
                      >
                        <span className="btn-indicator-label">{STANDARD_BUTTON_NAMES[i] ?? i}</span>
                        {btn.pressed && (
                          <span className="btn-value">{Math.round(btn.value * 100)}%</span>
                        )}
                      </div>
                    ))}
                  </div>
                </div>

                {/* Axes */}
                <div className="test-section">
                  <h4 className="test-section-label">
                    <span className="kicker">// Axes ({axes.length})</span>
                  </h4>
                  <div className="axes-list" role="group" aria-label="Axes">
                    {axes.map((val, i) => (
                      <div key={i} className="axis-row">
                        <span className="axis-name">{AXIS_NAMES[i] ?? `Axis ${i}`}</span>
                        <AxisBar value={val} />
                        <span className="axis-value" aria-hidden="true">{val.toFixed(3)}</span>
                      </div>
                    ))}
                    {axes.length === 0 && (
                      <p className="axes-empty">No axis data — press a button on the controller first.</p>
                    )}
                  </div>
                </div>
              </>
            )}
          </section>
        )}

        {/* ─── TAB: Input Bindings ─── */}
        {tab === 'Input Bindings' && (
          <section className="controller-section reveal-3" aria-label="Input binding editor">
            <div className="bindings-toolbar">
              <h3 className="section-sub-heading">Input Bindings</h3>
              <div className="bindings-toolbar-actions">
                {listening && (
                  <span className="listening-badge" role="status" aria-live="polite">
                    Press a button / move an axis…
                  </span>
                )}
                <button className="btn btn-secondary btn-sm" onClick={cancelListening} disabled={!listening}>
                  Cancel
                </button>
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={resetBindings}
                  aria-label="Reset all bindings to defaults"
                >
                  Reset Defaults
                </button>
              </div>
            </div>

            {gamepads.length === 0 && (
              <div className="bindings-no-gp-notice" role="status">
                Bindings are editable without a controller, but live-rebinding requires a connected gamepad.
              </div>
            )}

            <div className="bindings-table" role="table" aria-label="Input bindings">
              <div className="bindings-header" role="row">
                <span role="columnheader">Action</span>
                <span role="columnheader">Binding</span>
                <span role="columnheader">Remap</span>
              </div>

              {Object.entries(ACTION_LABELS).map(([action, label]) => {
                const isListening = listening === action;
                const bound = bindings[action];

                return (
                  <div
                    key={action}
                    className={`binding-row${isListening ? ' binding-listening' : ''}`}
                    role="row"
                  >
                    <span className="binding-action" role="cell">{label}</span>
                    <span className="binding-current" role="cell">
                      <span className={`binding-chip${!bound ? ' unbound' : ''}`}>
                        {isListening ? '…waiting…' : bindingLabel(bound)}
                      </span>
                    </span>
                    <div className="binding-actions" role="cell">
                      <button
                        className={`btn btn-sm ${isListening ? 'btn-primary' : 'btn-secondary'}`}
                        onClick={() => isListening ? cancelListening() : startListening(action)}
                        aria-label={`Remap ${label}`}
                        aria-pressed={isListening}
                      >
                        {isListening ? 'Listening…' : 'Remap'}
                      </button>
                      <button
                        className="btn btn-secondary btn-sm"
                        onClick={() => clearBinding(action)}
                        disabled={!bound}
                        aria-label={`Clear binding for ${label}`}
                      >
                        Clear
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>

            <p className="bindings-note">
              Bindings are saved to browser local storage and restored automatically.
            </p>
          </section>
        )}

      </div>
    </Layout>
  );
}
