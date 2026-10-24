import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

// The Tauri bridge does not exist in jsdom; tests answer what they call (`./tauri`).
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
  convertFileSrc: (p: string) => `asset://localhost/${encodeURIComponent(p)}`,
}));
vi.mock("@tauri-apps/api/event", async () => ({ listen: (await import("./tauri")).listen }));
// Files dragged onto the window arrive as `test://drag-drop` (`dragDrop` in `./tauri`).
vi.mock("@tauri-apps/api/webview", async () => {
  const { listen } = await import("./tauri");
  return { getCurrentWebview: () => ({ onDragDropEvent: (h: (e: { payload: unknown }) => void) => listen("test://drag-drop", h) }) };
});

// What WebView2 has and jsdom lacks.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};
Element.prototype.scrollTo ??= function () {};

afterEach(async () => {
  cleanup();
  (await import("./tauri")).resetTauri();
});
