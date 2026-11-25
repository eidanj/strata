import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { inTauri } from "./api";

/**
 * Native file drag-and-drop for the whole window.
 *
 * Tauri delivers drops through its own event (with real file paths), not
 * through HTML5 drop events. On Windows, HTML5 drag events don't fire inside
 * the webview while this is enabled, so the drop target is "whichever tab is
 * open" rather than a specific element.
 */
export function useFileDrop(onDrop: (paths: string[]) => void, enabled = true): boolean {
  const [hovering, setHovering] = useState(false);
  const handler = useRef(onDrop);
  handler.current = onDrop;

  useEffect(() => {
    if (!inTauri || !enabled) {
      setHovering(false);
      return;
    }
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "enter" || p.type === "over") setHovering(true);
        else if (p.type === "leave") setHovering(false);
        else if (p.type === "drop") {
          setHovering(false);
          if (p.paths.length > 0) handler.current(p.paths);
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [enabled]);

  return hovering;
}
