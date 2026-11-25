import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, inTauri, type Kind, type Progress, type SearchHit, type Sort } from "../lib/api";
import { compact, initials } from "../lib/format";
import { useNotify } from "../lib/notices";

interface Props {
  kind: Kind;
  noun: string;
  gameVersion: string;
  installedProjects: Set<string>;
  onInstalled: () => void;
}

interface Pending {
  pct: number | null;
  label: string;
}

const PAGE = 20;

export function ModrinthBrowser({ kind, noun, gameVersion, installedProjects, onInstalled }: Props) {
  const notify = useNotify();
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("relevance");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<Record<string, Pending>>({});
  const [justInstalled, setJustInstalled] = useState<Set<string>>(new Set());
  const searchId = useRef(0);

  // Search as you type, debounced; stale responses are dropped.
  useEffect(() => {
    const id = ++searchId.current;
    setLoading(true);
    const t = setTimeout(async () => {
      try {
        const res = await api.search({ kind, query, sort, offset: 0, limit: PAGE });
        if (id !== searchId.current) return;
        setHits(res.hits);
        setTotal(res.total_hits);
        setError(null);
      } catch (e) {
        if (id === searchId.current) setError(errorText(e));
      } finally {
        if (id === searchId.current) setLoading(false);
      }
    }, 280);
    return () => clearTimeout(t);
  }, [kind, query, sort]);

  // Download progress for anything installing from this tab.
  useEffect(() => {
    if (!inTauri) return;
    const un = listen<Progress>("install-progress", ({ payload: p }) => {
      setPending((all) => {
        if (!(p.requestId in all)) return all;
        const pct = p.stage === "downloading" && p.total > 0 ? Math.round((p.received / p.total) * 100) : null;
        const label = p.projectId === p.requestId ? "" : p.title;
        return { ...all, [p.requestId]: { pct, label } };
      });
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  async function loadMore() {
    setLoading(true);
    try {
      const res = await api.search({ kind, query, sort, offset: hits.length, limit: PAGE });
      setHits((h) => [...h, ...res.hits]);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }

  async function install(hit: SearchHit) {
    setPending((p) => ({ ...p, [hit.project_id]: { pct: null, label: "" } }));
    try {
      const report = await api.install(kind, hit.project_id);
      setJustInstalled((s) => new Set(s).add(hit.project_id));
      const deps = report.installed.filter((i) => i.dependency).map((i) => i.title);
      const main = report.installed.find((i) => !i.dependency);
      const lines = [
        ...(main?.replaced ? [`Replaced ${main.replaced}`] : []),
        ...(deps.length ? [`Also installed ${deps.join(", ")} (required)`] : []),
        ...report.warnings,
      ];
      notify(report.warnings.length ? "warn" : "ok", `Installed ${hit.title}${main ? ` ${main.versionNumber}` : ""}`, lines);
      onInstalled();
    } catch (e) {
      notify("error", `Couldn't install ${hit.title}`, [errorText(e)]);
    } finally {
      setPending(({ [hit.project_id]: _, ...rest }) => rest);
    }
  }

  return (
    <div>
      <div className="searchbar">
        <input
          className="field field--grow"
          type="search"
          placeholder={`Search Modrinth for ${noun} on ${gameVersion}`}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          aria-label={`Search Modrinth ${noun}`}
        />
        <select className="field" value={sort} onChange={(e) => setSort(e.target.value as Sort)} aria-label="Sort by">
          <option value="relevance">Best match</option>
          <option value="downloads">Most downloaded</option>
          <option value="follows">Most followed</option>
          <option value="updated">Recently updated</option>
          <option value="newest">Newest</option>
        </select>
      </div>
      <p className="search-meta">
        {loading ? "Searching…" : error ? "" : `${compact(total)} results that support ${gameVersion}`}
      </p>

      {error && (
        <div className="note">
          <p>{error}</p>
        </div>
      )}

      <ul className="list">
        {hits.map((hit) => {
          const isInstalled = installedProjects.has(hit.project_id) || justInstalled.has(hit.project_id);
          const job = pending[hit.project_id];
          return (
            <li key={hit.project_id} className="row">
              {hit.icon_url ? (
                <img className="row__icon" src={hit.icon_url} alt="" loading="lazy" />
              ) : (
                <div className="row__icon row__icon--blank" aria-hidden="true">
                  {initials(hit.title)}
                </div>
              )}
              <div className="row__text">
                <div className="row__title">
                  <span className="row__name">{hit.title}</span>
                  <span className="row__version">by {hit.author}</span>
                </div>
                <div className="row__desc">{hit.description}</div>
              </div>
              <div className="row__side">
                <span className="row__version">{compact(hit.downloads)} downloads</span>
                {job ? (
                  <div className="progress" title={job.label ? `Installing ${job.label}` : undefined}>
                    <div className="progress__fill" style={{ width: `${job.pct ?? 8}%` }} />
                    <span>{job.pct === null ? "Preparing" : `${job.pct}%`}</span>
                  </div>
                ) : isInstalled ? (
                  <span className="tag tag--tone">Installed</span>
                ) : (
                  <button className="btn btn--primary" onClick={() => install(hit)}>
                    Install
                  </button>
                )}
              </div>
            </li>
          );
        })}
      </ul>

      {!loading && !error && hits.length === 0 && (
        <div className="empty">
          <p className="empty__title">Nothing on Modrinth matches “{query}” for {gameVersion}</p>
          <p>Try a shorter search. Some projects haven't released a {gameVersion} build yet.</p>
        </div>
      )}

      {hits.length < total && (
        <div className="more">
          <button className="btn" onClick={loadMore} disabled={loading}>
            Show more
          </button>
        </div>
      )}
    </div>
  );
}
