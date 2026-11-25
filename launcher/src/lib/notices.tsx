import { createContext, useCallback, useContext, useState, type ReactNode } from "react";

export type NoticeTone = "ok" | "warn" | "error";
interface Notice {
  id: number;
  tone: NoticeTone;
  title: string;
  lines?: string[];
}

const Ctx = createContext<(tone: NoticeTone, title: string, lines?: string[]) => void>(() => {});

let next = 1;

export function NoticeProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<Notice[]>([]);

  const push = useCallback((tone: NoticeTone, title: string, lines?: string[]) => {
    const id = next++;
    setItems((list) => [...list.slice(-3), { id, tone, title, lines }]);
    const ttl = tone === "error" ? 9000 : 5500;
    setTimeout(() => setItems((list) => list.filter((n) => n.id !== id)), ttl);
  }, []);

  return (
    <Ctx.Provider value={push}>
      {children}
      <div className="notices" role="status" aria-live="polite">
        {items.map((n) => (
          <div key={n.id} className={`notice notice--${n.tone}`}>
            <p className="notice__title">{n.title}</p>
            {n.lines && n.lines.length > 0 && (
              <ul className="notice__body">
                {n.lines.map((l, i) => (
                  <li key={i}>{l}</li>
                ))}
              </ul>
            )}
          </div>
        ))}
      </div>
    </Ctx.Provider>
  );
}

export const useNotify = () => useContext(Ctx);
