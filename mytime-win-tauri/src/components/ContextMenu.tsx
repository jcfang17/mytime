import { CATEGORY_INFO } from "../types";
import type { Category } from "../types";

export type ContextMenuState =
  | { kind: "app"; x: number; y: number; appName: string }
  | { kind: "context"; x: number; y: number; appName: string; context: string };

interface ContextMenuProps {
  menu: ContextMenuState;
  onSelect: (category: string) => void;
}

export function ContextMenu({ menu, onSelect }: ContextMenuProps) {
  return (
    <div
      className="context-menu"
      style={{ left: menu.x, top: menu.y }}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="context-header">Set Category</div>
      {(Object.keys(CATEGORY_INFO) as Category[])
        .filter((cat) => cat !== "unknown")
        .map((cat) => {
          const info = CATEGORY_INFO[cat];
          return (
            <button
              key={cat}
              className="context-item"
              onClick={() => onSelect(cat)}
            >
              {info.emoji} {info.label}
            </button>
          );
        })}
    </div>
  );
}
