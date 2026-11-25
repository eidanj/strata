import type { ContentItem, Kind } from "../lib/api";
import { initials } from "../lib/format";

interface Props {
  kind: Kind;
  items: ContentItem[];
  busy: string | null;
  onToggle: (item: ContentItem) => void;
  onRemove: (item: ContentItem) => void;
  onMove: (item: ContentItem, delta: -1 | 1) => void;
  onUseShader: (file: string | null) => void;
}

const sourceLabel: Record<ContentItem["source"], string> = {
  core: "Bundled",
  modrinth: "Modrinth",
  local: "Added by you",
};

function Icon({ item }: { item: Pick<ContentItem, "icon" | "title"> }) {
  return item.icon ? (
    <img className="row__icon" src={item.icon} alt="" />
  ) : (
    <div className="row__icon row__icon--blank" aria-hidden="true">
      {initials(item.title)}
    </div>
  );
}

export function InstalledList({ kind, items, busy, onToggle, onRemove, onMove, onUseShader }: Props) {
  const enabledPacks = items.filter((i) => i.priority !== null).length;
  const activeShader = items.find((i) => i.enabled)?.file ?? null;

  return (
    <ul className="list">
      {kind === "shader" && items.length > 0 && (
        <li className={`row ${activeShader ? "row--off" : ""}`}>
          <div className="row__icon row__icon--blank" aria-hidden="true">–</div>
          <div className="row__text">
            <div className="row__title">
              <span className="row__name">No shaders</span>
            </div>
            <div className="row__desc">Best frame rate. Iris stays installed but does nothing.</div>
          </div>
          <div className="row__side">
            {activeShader === null ? (
              <span className="tag tag--tone">In use</span>
            ) : (
              <button className="btn" onClick={() => onUseShader(null)} disabled={busy !== null}>
                Use
              </button>
            )}
          </div>
        </li>
      )}

      {items.map((item) => {
        const working = busy === item.file;
        return (
          <li key={item.file} className={`row ${item.enabled ? "" : "row--off"}`}>
            <Icon item={item} />

            <div className="row__text">
              <div className="row__title">
                <span className="row__name" title={item.file}>
                  {item.title}
                </span>
                {item.version && <span className="row__version">{item.version}</span>}
              </div>
              <div className="row__desc">{item.description ?? item.file}</div>
            </div>

            <div className="row__side">
              {item.compatibility === "incompatible" && (
                <span className="tag tag--bad" title={`This mod declares Minecraft ${item.minecraft}`}>
                  Built for {item.minecraft}
                </span>
              )}
              <span className="tag">{sourceLabel[item.source]}</span>

              {kind === "resourcepack" && item.priority !== null && (
                <>
                  <span className="row__rank" aria-label={`Priority ${item.priority + 1}`}>
                    {item.priority + 1}
                  </span>
                  <button
                    className="btn btn--quiet btn--icon"
                    aria-label={`Move ${item.title} up`}
                    disabled={item.priority === 0 || busy !== null}
                    onClick={() => onMove(item, -1)}
                  >
                    ↑
                  </button>
                  <button
                    className="btn btn--quiet btn--icon"
                    aria-label={`Move ${item.title} down`}
                    disabled={item.priority === enabledPacks - 1 || busy !== null}
                    onClick={() => onMove(item, 1)}
                  >
                    ↓
                  </button>
                </>
              )}

              {kind === "shader" ? (
                item.enabled ? (
                  <span className="tag tag--tone">In use</span>
                ) : (
                  <button className="btn" onClick={() => onUseShader(item.file)} disabled={busy !== null}>
                    Use
                  </button>
                )
              ) : (
                <button
                  className="switch"
                  role="switch"
                  aria-checked={item.enabled}
                  aria-label={`${item.enabled ? "Turn off" : "Turn on"} ${item.title}`}
                  disabled={working}
                  onClick={() => onToggle(item)}
                />
              )}

              <button
                className="btn btn--quiet btn--icon row__remove"
                aria-label={`Remove ${item.title}`}
                title="Remove"
                disabled={working}
                onClick={() => onRemove(item)}
              >
                ✕
              </button>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
