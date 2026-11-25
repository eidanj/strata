import { useEffect, useMemo, useState, type ReactNode } from "react";
import { api, errorText, type CrosshairConfig, type CrosshairStyle, type HudConfig } from "../lib/api";
import { cooldownRects, crosshairRects, type Rect } from "../lib/crosshair";
import { joinArgb, splitArgb } from "../lib/format";
import { useNotify } from "../lib/notices";
import type { Layer } from "../layers";

const STYLES: { id: CrosshairStyle; name: string }[] = [
  { id: "plus", name: "Plus" },
  { id: "t", name: "T" },
  { id: "x", name: "X" },
  { id: "circle", name: "Circle" },
  { id: "dot", name: "Dot" },
];

const MODULES: { id: string; name: string; about: string }[] = [
  { id: "fps", name: "FPS", about: "Frames per second, updated twice a second." },
  { id: "cps", name: "CPS", about: "Left and right clicks per second." },
  { id: "ping", name: "Ping", about: "Your latency to the server in milliseconds." },
  { id: "keystrokes", name: "Keystrokes", about: "W, A, S, D, both mouse buttons and jump." },
  { id: "armor", name: "Armor status", about: "Each armor piece with its durability." },
  { id: "held", name: "Held items", about: "Main-hand and off-hand items, durability and how many you carry." },
  { id: "togglesprint", name: "Toggle sprint", about: "Tap sprint once to keep sprinting. Shows the current state." },
  { id: "coords", name: "Coordinates", about: "Your position and the direction you face." },
];

export function HudTab({ layer }: { layer: Layer }) {
  const notify = useNotify();
  const [saved, setSaved] = useState<HudConfig | null>(null);
  const [draft, setDraft] = useState<HudConfig | null>(null);
  const [onTarget, setOnTarget] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api
      .hud()
      .then((c) => {
        setSaved(c);
        setDraft(structuredClone(c));
      })
      .catch((e) => notify("error", "Couldn't read the HUD settings", [errorText(e)]));
  }, [notify]);

  const dirty = useMemo(() => JSON.stringify(saved) !== JSON.stringify(draft), [saved, draft]);

  if (!draft) return null;
  const c = draft.crosshair;

  const setCross = (patch: Partial<CrosshairConfig>) => setDraft({ ...draft, crosshair: { ...c, ...patch } });
  const setModule = (id: string, patch: Record<string, unknown>) =>
    setDraft({ ...draft, modules: { ...draft.modules, [id]: { ...draft.modules[id]!, ...patch } } });

  async function save() {
    if (!draft) return;
    setSaving(true);
    try {
      await api.saveHud(draft);
      setSaved(structuredClone(draft));
      notify("ok", "Saved HUD", ["Changes show the next time the game starts."]);
    } catch (e) {
      notify("error", "Couldn't save the HUD", [errorText(e)]);
    } finally {
      setSaving(false);
    }
  }

  async function reset() {
    const fresh = await api.defaultHud();
    setDraft(fresh);
  }

  return (
    <>
      <header className="stage__head">
        <div className="stage__titles">
          <h1 className="stage__title">{layer.name}</h1>
          <p className="stage__blurb">{layer.blurb}</p>
        </div>
      </header>

      <div className="stage__body">
        <div className="hud">
          <div className="scope">
            <CrosshairPreview c={c} onTarget={onTarget} />
            <p className="scope__caption">Shown four times actual size. The small one in the corner is 1:1.</p>
            <div className="control" style={{ gridTemplateColumns: "1fr auto" }}>
              <span className="control__label">Preview aiming at a player</span>
              <button className="switch" role="switch" aria-checked={onTarget} onClick={() => setOnTarget(!onTarget)} />
            </div>
          </div>

          <div className="controls">
            <section>
              <h2 className="group__title">Crosshair</h2>
              <Row label="Use Strata crosshair">
                <button className="switch" role="switch" aria-checked={c.enabled} onClick={() => setCross({ enabled: !c.enabled })} />
              </Row>
              <Row label="Shape">
                <div className="styles">
                  {STYLES.map((s) => (
                    <button key={s.id} className="btn" aria-pressed={c.style === s.id} onClick={() => setCross({ style: s.id })}>
                      {s.name}
                    </button>
                  ))}
                </div>
              </Row>
              {c.style !== "dot" && (
                <>
                  <Slider label="Length" value={c.length} min={1} max={20} onChange={(v) => setCross({ length: v })} />
                  <Slider label="Gap" value={c.gap} min={0} max={12} onChange={(v) => setCross({ gap: v })} />
                  <Slider label="Thickness" value={c.thickness} min={1} max={6} onChange={(v) => setCross({ thickness: v })} />
                  <Row label="Centre dot">
                    <button className="switch" role="switch" aria-checked={c.dot} onClick={() => setCross({ dot: !c.dot })} />
                  </Row>
                </>
              )}
              {(c.dot || c.style === "dot") && (
                <Slider label="Dot size" value={c.dotSize} min={1} max={8} onChange={(v) => setCross({ dotSize: v })} />
              )}
              <ColorRow label="Colour" value={c.color} onChange={(v) => setCross({ color: v })} />
              <Row label="Outline">
                <button className="switch" role="switch" aria-checked={c.outline} onClick={() => setCross({ outline: !c.outline })} />
              </Row>
              {c.outline && <ColorRow label="Outline colour" value={c.outlineColor} onChange={(v) => setCross({ outlineColor: v })} />}
              <Row label="Change colour on target">
                <button
                  className="switch"
                  role="switch"
                  aria-checked={c.colorOnTarget}
                  onClick={() => setCross({ colorOnTarget: !c.colorOnTarget })}
                />
              </Row>
              {c.colorOnTarget && <ColorRow label="Target colour" value={c.targetColor} onChange={(v) => setCross({ targetColor: v })} />}
              <Row label="Attack cooldown bar">
                <button
                  className="switch"
                  role="switch"
                  aria-checked={c.cooldownIndicator}
                  onClick={() => setCross({ cooldownIndicator: !c.cooldownIndicator })}
                />
              </Row>
            </section>

            <section>
              <h2 className="group__title">Overlays</h2>
              <p className="stage__blurb" style={{ margin: "0 0 8px" }}>
                Press Right Shift in game to drag these into place and scroll over one to resize it.
              </p>
              <ul className="list">
                {MODULES.map((m) => {
                  const mod = draft.modules[m.id];
                  if (!mod) return null;
                  return (
                    <li key={m.id} className={`row ${mod.enabled ? "" : "row--off"}`} style={{ gridTemplateColumns: "1fr auto" }}>
                      <div className="row__text">
                        <div className="row__title">
                          <span className="row__name">{m.name}</span>
                          <span className="row__version">{Math.round(mod.scale * 100)}%</span>
                        </div>
                        <div className="row__desc">{m.about}</div>
                      </div>
                      <div className="row__side">
                        <input
                          type="range"
                          min={0.5}
                          max={2}
                          step={0.05}
                          value={mod.scale}
                          aria-label={`${m.name} size`}
                          style={{ width: 110, accentColor: "var(--tone)" }}
                          onChange={(e) => setModule(m.id, { scale: Number(e.target.value) })}
                        />
                        <button
                          className="switch"
                          role="switch"
                          aria-checked={mod.enabled}
                          aria-label={`Show ${m.name}`}
                          onClick={() => setModule(m.id, { enabled: !mod.enabled })}
                        />
                      </div>
                    </li>
                  );
                })}
              </ul>
            </section>
          </div>
        </div>

        <div className="savebar">
          {dirty && <span>Unsaved changes</span>}
          <button className="btn btn--quiet" onClick={reset}>
            Reset to defaults
          </button>
          <button className="btn btn--primary" disabled={!dirty || saving} onClick={save}>
            {saving ? "Saving…" : "Save HUD"}
          </button>
        </div>
      </div>
    </>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="control" style={{ gridTemplateColumns: "170px 1fr" }}>
      <span className="control__label">{label}</span>
      <div style={{ justifySelf: "end" }}>{children}</div>
    </div>
  );
}

function Slider({ label, value, min, max, onChange }: { label: string; value: number; min: number; max: number; onChange: (v: number) => void }) {
  return (
    <label className="control">
      <span className="control__label">{label}</span>
      <input type="range" min={min} max={max} step={1} value={value} onChange={(e) => onChange(Number(e.target.value))} />
      <span className="control__value">{value}px</span>
    </label>
  );
}

function ColorRow({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  const { hex, alpha } = splitArgb(value);
  return (
    <div className="control">
      <span className="control__label">{label}</span>
      <input
        type="range"
        min={0}
        max={1}
        step={0.05}
        value={alpha}
        aria-label={`${label} opacity`}
        onChange={(e) => onChange(joinArgb(hex, Number(e.target.value)))}
      />
      <input type="color" value={hex} aria-label={label} onChange={(e) => onChange(joinArgb(e.target.value, alpha))} />
    </div>
  );
}

function rgba(argb: string): string {
  const { hex, alpha } = splitArgb(argb);
  return `color-mix(in srgb, ${hex} ${Math.round(alpha * 100)}%, transparent)`;
}

function CrosshairPreview({ c, onTarget }: { c: CrosshairConfig; onTarget: boolean }) {
  const rects = c.enabled ? crosshairRects(c) : [];
  const color = onTarget && c.colorOnTarget ? c.targetColor : c.color;
  const cool = c.enabled && c.cooldownIndicator ? cooldownRects(c, 0.62) : null;
  const all: Rect[] = cool ? [...rects, cool.back] : rects;

  // Draw in a 64x64 pixel space centred on 32,32.
  const draw = (scale: number, ox: number, oy: number) => (
    <g transform={`translate(${ox} ${oy}) scale(${scale})`} shapeRendering="crispEdges">
      {c.outline &&
        all.map((r, i) => (
          <rect key={`o${i}`} x={r.x0 - 1} y={r.y0 - 1} width={r.x1 - r.x0 + 2} height={r.y1 - r.y0 + 2} fill={rgba(c.outlineColor)} />
        ))}
      {cool && <rect x={cool.back.x0} y={cool.back.y0} width={cool.back.x1 - cool.back.x0} height={cool.back.y1 - cool.back.y0} fill="rgb(0 0 0 / 0.45)" />}
      {cool && <rect x={cool.fill.x0} y={cool.fill.y0} width={cool.fill.x1 - cool.fill.x0} height={cool.fill.y1 - cool.fill.y0} fill={rgba(color)} />}
      {rects.map((r, i) => (
        <rect key={i} x={r.x0} y={r.y0} width={r.x1 - r.x0} height={r.y1 - r.y0} fill={rgba(color)} />
      ))}
    </g>
  );

  return (
    <div className="scope__view">
      <svg viewBox="0 0 256 256" role="img" aria-label="Crosshair preview">
        {draw(4, 128, 128)}
        <rect x={212} y={212} width={36} height={36} fill="rgb(0 0 0 / 0.22)" />
        {draw(1, 230, 230)}
      </svg>
    </div>
  );
}
