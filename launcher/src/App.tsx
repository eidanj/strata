import { useCallback, useEffect, useMemo, useState } from "react";
import { api, errorText, type Kind, type Overview } from "./lib/api";
import { useFileDrop } from "./lib/useFileDrop";
import { useNotify } from "./lib/notices";
import { LAYERS, type LayerId } from "./layers";
import { CoreNav } from "./components/CoreNav";
import { ContentTab } from "./components/ContentTab";
import { DropVeil } from "./components/DropVeil";
import { HudTab } from "./tabs/HudTab";
import { ProfilesTab } from "./tabs/ProfilesTab";

export function App() {
  const notify = useNotify();
  const [active, setActive] = useState<LayerId>(initialLayer);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [counts, setCounts] = useState<Partial<Record<LayerId, string>>>({});
  const [refresh, setRefresh] = useState(0);

  const layer = LAYERS.find((l) => l.id === active)!;

  const loadOverview = useCallback(() => {
    api.overview().then(setOverview).catch((e) => notify("error", "Couldn't load the instance", [errorText(e)]));
  }, [notify]);

  useEffect(() => {
    loadOverview();
    // Sidebar counts for every content layer.
    for (const l of LAYERS) {
      if (!l.kind) continue;
      api
        .list(l.kind)
        .then((items) => setCounts((c) => ({ ...c, [l.id]: summary(l.kind!, items.length, items.filter((i) => i.enabled).length) })))
        .catch(() => {});
    }
  }, [loadOverview]);

  // One stable callback per layer, so ContentTab doesn't reload in a loop.
  const countSetters = useMemo(() => {
    const out = {} as Record<LayerId, (s: string) => void>;
    for (const l of LAYERS) out[l.id] = (s: string) => setCounts((c) => (c[l.id] === s ? c : { ...c, [l.id]: s }));
    return out;
  }, []);

  const importInto = useCallback(
    async (kind: Kind, paths: string[]) => {
      try {
        const r = await api.importFiles(kind, paths);
        const lines = [
          ...r.added.flatMap((a) => [
            a.replaced ? `${a.file} replaced ${a.replaced}` : a.file,
            ...(a.warning ? [a.warning] : []),
          ]),
          ...r.rejected.map((x) => `${x.file}: ${x.reason}`),
        ];
        const tone = r.rejected.length ? (r.added.length ? "warn" : "error") : r.added.some((a) => a.warning) ? "warn" : "ok";
        const title = r.added.length
          ? `Added ${r.added.length} ${r.added.length === 1 ? "file" : "files"} to ${layer.name}`
          : `Nothing added to ${layer.name}`;
        notify(tone, title, lines);
        setRefresh((n) => n + 1);
        loadOverview();
      } catch (e) {
        notify("error", "Couldn't add those files", [errorText(e)]);
      }
    },
    [layer.name, notify, loadOverview],
  );

  const hovering = useFileDrop((paths) => {
    if (layer.kind) importInto(layer.kind, paths);
    else notify("warn", "Open Mods, Resource packs or Shaders first", ["Files are added to the section you drop them on."]);
  });

  return (
    <div className="app">
      <CoreNav active={active} onSelect={setActive} counts={counts} instance={overview?.instance ?? null} />
      <main className="stage" data-layer={active}>
        {layer.kind ? (
          <ContentTab
            key={layer.id}
            layer={{ ...layer, kind: layer.kind }}
            overview={overview}
            refreshKey={refresh}
            onImport={(paths) => importInto(layer.kind!, paths)}
            onCount={countSetters[layer.id]}
            onChanged={loadOverview}
          />
        ) : active === "hud" ? (
          <HudTab layer={layer} />
        ) : (
          <ProfilesTab layer={layer} overview={overview} onChanged={loadOverview} />
        )}
        {hovering && layer.kind && <DropVeil target={layer.name} hint={layer.dropHint} />}
      </main>
    </div>
  );
}

/** `?tab=hud` opens a section directly (useful when previewing in a browser). */
function initialLayer(): LayerId {
  const wanted = new URLSearchParams(window.location.search).get("tab");
  return LAYERS.some((l) => l.id === wanted) ? (wanted as LayerId) : "mods";
}

function summary(kind: Kind, total: number, on: number): string {
  return kind === "shader" ? `${total} installed` : `${on} on, ${total} total`;
}
