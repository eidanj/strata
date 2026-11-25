import { useEffect, useState } from "react";
import { api, errorText, type Overview, type Profile } from "../lib/api";
import { useNotify } from "../lib/notices";
import type { Layer } from "../layers";

const OPTION_NAMES: Record<string, string> = {
  renderDistance: "Render distance",
  simulationDistance: "Simulation distance",
  maxFps: "Frame cap",
};

function describeOption(key: string, value: number | boolean): string | null {
  const name = OPTION_NAMES[key];
  if (!name) return null;
  if (key === "maxFps") return `${name} ${value === 260 ? "unlimited" : value}`;
  return `${name} ${value} chunks`;
}

export function ProfilesTab({ layer, overview, onChanged }: { layer: Layer; overview: Overview | null; onChanged: () => void }) {
  const notify = useNotify();
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const active = overview?.instance.activeProfile;

  useEffect(() => {
    api.profiles().then(setProfiles).catch((e) => notify("error", "Couldn't read profiles", [errorText(e)]));
  }, [notify]);

  async function choose(p: Profile) {
    try {
      await api.setProfile(p.id);
      onChanged();
    } catch (e) {
      notify("error", `Couldn't switch to ${p.name}`, [errorText(e)]);
    }
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
        <div role="radiogroup" aria-label="Performance profile">
          {profiles.map((p) => (
            <button key={p.id} className="profile" role="radio" aria-checked={p.id === active} onClick={() => choose(p)}>
              <span className="profile__dot" />
              <span>
                <span className="profile__name">{p.name}</span>
                <p className="profile__desc">{p.description}</p>
                <span className="profile__specs">
                  <span>{(p.memoryMb / 1024).toFixed(p.memoryMb % 1024 ? 1 : 0)} GB memory</span>
                  {Object.entries(p.options)
                    .map(([k, v]) => describeOption(k, v))
                    .filter(Boolean)
                    .map((t) => (
                      <span key={t}>{t}</span>
                    ))}
                  <code>{p.jvmArgs.join(" ")}</code>
                </span>
              </span>
            </button>
          ))}
        </div>
      </div>
    </>
  );
}
