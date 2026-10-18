// jsdom lays nothing out, so the real virtualizer would show no rows. Tests that render a
// transcript replace it with this one, which shows every row:
//   vi.mock("@tanstack/react-virtual", () => import("../test/virtual"));

import { vi } from "vitest";

export const scrollToIndex = vi.fn();

export function useVirtualizer(o: { count: number; getItemKey: (i: number) => string | number }) {
  return {
    getVirtualItems: () => Array.from({ length: o.count }, (_, index) => ({ index, key: o.getItemKey(index), start: index * 30 })),
    getTotalSize: () => o.count * 30,
    measureElement: () => {},
    scrollToIndex,
  };
}
