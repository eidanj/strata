import type { CSSProperties } from "react";
import type { Instance } from "../lib/api";
import { LAYERS, type LayerId } from "../layers";
import logo from "../../src-tauri/icons/128x128.png";

interface Props {
  active: LayerId;
  onSelect: (id: LayerId) => void;
  counts: Partial<Record<LayerId, string>>;
  instance: Instance | null;
}

/** The sidebar is a drilled core sample: each section is one stratum. */
export function CoreNav({ active, onSelect, counts, instance }: Props) {
  return (
    <nav className="core" aria-label="Sections">
      <div className="core__mark">
        <img src={logo} alt="" />
        <span className="core__word">Strata</span>
      </div>

      <div className="core__sample">
        {LAYERS.map((layer) => (
          <button
            key={layer.id}
            type="button"
            className="layer"
            aria-current={layer.id === active ? "page" : undefined}
            onClick={() => onSelect(layer.id)}
          >
            <span className="layer__band" style={{ "--band": layer.tone, height: layer.band } as CSSProperties} />
            <span className="layer__label">
              <span className="layer__name">{layer.name}</span>
              {counts[layer.id] && <span className="layer__meta">{counts[layer.id]}</span>}
            </span>
          </button>
        ))}
      </div>

      <div className="core__foot">
        <div className="core__instance">{instance?.name ?? "Loading"}</div>
        {instance && (
          <div className="core__version">
            Fabric {instance.loaderVersion} on {instance.minecraftVersion}
          </div>
        )}
      </div>
    </nav>
  );
}
