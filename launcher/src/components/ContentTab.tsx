import { useCallback, useEffect, useMemo, useState } from "react";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { api, errorText, inTauri, type ContentItem, type Kind, type Overview } from "../lib/api";
import { useNotify } from "../lib/notices";
import type { Layer } from "../layers";
import { InstalledList } from "./InstalledList";
import { ModrinthBrowser } from "./ModrinthBrowser";

interface Props {
  layer: Layer & { kind: Kind };
  overview: Overview | null;
  refreshKey: number;
  onImport: (paths: string[]) => void;
  onCount: (count: string) => void;
  onChanged: () => void;
}

const NOUNS: Record<Kind, string> = { mod: "mods", resourcepack: "resource packs", shader: "shader packs" };
const FILTERS: Record<Kind, { name: string; extensions: string[] }> = {
  mod: { name: "Fabric mods", extensions: ["jar"] },
  resourcepack: { name: "Resource packs", extensions: ["zip"] },
  shader: { name: "Shader packs", extensions: ["zip"] },
};

export function ContentTab({ layer, overview, refreshKey, onImport, onCount, onChanged }: Props) {
  const kind = layer.kind;
  const notify = useNotify();
  const [view, setView] = useState<"installed" | "browse">("installed");
  const [items, setItems] = useState<ContentItem[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const list = await api.list(kind);
      setItems(list);
      const on = list.filter((i) => i.enabled).length;
      onCount(kind === "shader" ? `${list.length} installed` : `${on} on, ${list.length} total`);
    } catch (e) {
      notify("error", `Couldn't read ${NOUNS[kind]}`, [errorText(e)]);
      setItems([]);
    }
  }, [kind, notify, onCount]);

  useEffect(() => {
    setItems(null);
    setView("installed");
  }, [kind]);

  useEffect(() => {
    load();
  }, [load, refreshKey]);

  const installedProjects = useMemo(
    () => new Set((items ?? []).map((i) => i.projectId).filter((p): p is string => !!p)),
    [items],
  );

  async function run(file: string, action: () => Promise<void>, failure: string) {
    setBusy(file);
    try {
      await action();
      await load();
      onChanged();
    } catch (e) {
      notify("error", failure, [errorText(e)]);
    } finally {
      setBusy(null);
    }
  }

  const enabledPackOrder = () =>
    (items ?? [])
      .filter((i) => i.priority !== null)
      .sort((a, b) => a.priority! - b.priority!)
      .map((i) => i.file);

  function toggle(item: ContentItem) {
    if (kind === "mod") {
      run(item.file, () => api.setModEnabled(item.file, !item.enabled), `Couldn't change ${item.title}`);
    } else if (kind === "resourcepack") {
      const order = enabledPackOrder().filter((f) => f !== item.file);
      // A newly enabled pack goes on top, the same as in Minecraft.
      const next = item.enabled ? order : [item.file, ...order];
      run(item.file, () => api.setResourcePacks(next), `Couldn't change ${item.title}`);
    }
  }

  function move(item: ContentItem, delta: -1 | 1) {
    const order = enabledPackOrder();
    const from = order.indexOf(item.file);
    const to = from + delta;
    if (from < 0 || to < 0 || to >= order.length) return;
    [order[from], order[to]] = [order[to]!, order[from]!];
    run(item.file, () => api.setResourcePacks(order), "Couldn't reorder packs");
  }

  async function remove(item: ContentItem) {
    const message = `Remove ${item.title}? This deletes ${item.file} from the instance folder.`;
    const yes = inTauri ? await ask(message, { title: "Remove", kind: "warning", okLabel: "Remove" }) : confirm(message);
    if (!yes) return;
    run(item.file, () => api.remove(kind, item.file), `Couldn't remove ${item.title}`);
  }

  async function addFiles() {
    if (!inTauri) return;
    const picked = await open({ multiple: true, directory: false, filters: [FILTERS[kind]] });
    if (picked) onImport(Array.isArray(picked) ? picked : [picked]);
  }

  const count = items?.length ?? 0;
  const mc = overview?.instance.minecraftVersion ?? "26.3";

  return (
    <>
      <header className="stage__head">
        <div className="stage__titles">
          <h1 className="stage__title">{layer.name}</h1>
          <p className="stage__blurb">{layer.blurb}</p>
        </div>
        <div className="stage__actions">
          <button className="btn" onClick={addFiles}>
            Add files
          </button>
          <button className="btn btn--quiet" onClick={() => api.openFolder(kind).catch((e) => notify("error", "Couldn't open the folder", [errorText(e)]))}>
            Open folder
          </button>
        </div>
      </header>

      <div className="stage__body">
        <div className="tabs" role="tablist">
          <button className="tabs__item" role="tab" aria-selected={view === "installed"} onClick={() => setView("installed")}>
            Installed<span className="tabs__count">{count}</span>
          </button>
          <button className="tabs__item" role="tab" aria-selected={view === "browse"} onClick={() => setView("browse")}>
            Get from Modrinth
          </button>
        </div>

        {kind === "shader" && overview && !overview.irisInstalled && (
          <div className="note">
            <p>Shader packs need the Iris mod, which isn't installed in this instance.</p>
            <IrisButton onDone={onChanged} />
          </div>
        )}

        {view === "browse" ? (
          <ModrinthBrowser
            kind={kind}
            noun={NOUNS[kind]}
            gameVersion={mc}
            installedProjects={installedProjects}
            onInstalled={() => {
              load();
              onChanged();
            }}
          />
        ) : items === null ? null : items.length === 0 ? (
          <div className="empty">
            <p className="empty__title">No {NOUNS[kind]} yet</p>
            <p>
              Drag {layer.dropHint} into this window, or find some on Modrinth.
            </p>
            <button className="btn btn--primary" onClick={() => setView("browse")}>
              Browse Modrinth
            </button>
          </div>
        ) : (
          <InstalledList
            kind={kind}
            items={items}
            busy={busy}
            onToggle={toggle}
            onRemove={remove}
            onMove={move}
            onUseShader={(file) => run(file ?? "none", () => api.setActiveShader(file), "Couldn't switch shaders")}
          />
        )}
      </div>
    </>
  );
}

function IrisButton({ onDone }: { onDone: () => void }) {
  const notify = useNotify();
  const [working, setWorking] = useState(false);
  return (
    <button
      className="btn btn--primary"
      disabled={working}
      onClick={async () => {
        setWorking(true);
        try {
          await api.install("mod", "iris");
          notify("ok", "Installed Iris", ["Shader packs will load the next time the game starts."]);
          onDone();
        } catch (e) {
          notify("error", "Couldn't install Iris", [errorText(e)]);
        } finally {
          setWorking(false);
        }
      }}
    >
      {working ? "Installing Iris…" : "Install Iris"}
    </button>
  );
}
