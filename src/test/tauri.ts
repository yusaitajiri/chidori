// The Tauri bridge in tests: commands answered by their Rust names, events emitted by hand.

import { invoke } from "@tauri-apps/api/core";
import { vi } from "vitest";
import type { EventMap } from "../ipc/events";
import type { ErrorCode } from "../ipc/types";

type Args = Record<string, unknown>;
type Listener = (e: { payload: unknown }) => void;

const listeners = new Map<string, Set<Listener>>();

/** Stands in for `listen` from `@tauri-apps/api/event` (see `setup.ts`). */
export async function listen(name: string, handler: Listener): Promise<() => void> {
  const set = listeners.get(name) ?? new Set();
  set.add(handler);
  listeners.set(name, set);
  return () => set.delete(handler);
}

/** Sends an event to whatever the UI subscribed to it. */
export function emit<K extends keyof EventMap>(name: K, payload: EventMap[K]) {
  for (const l of [...(listeners.get(name) ?? [])]) l({ payload });
}

/** Drags files onto the window (`onDragDropEvent`, see `setup.ts`). */
export function dragDrop(payload: { type: "enter" | "drop"; paths: string[] } | { type: "leave" }) {
  for (const l of [...(listeners.get("test://drag-drop") ?? [])]) l({ payload });
}

/**
 * Answers commands by name: a function is called with the command's arguments, anything else is
 * the result. Commands left out resolve to `undefined`, as most of them do.
 */
export function answer(handlers: Record<string, unknown>) {
  const mock = vi.mocked(invoke);
  mock.mockReset();
  mock.mockImplementation(async (cmd: string, args?: unknown) => {
    const h = handlers[cmd];
    return typeof h === "function" ? h((args ?? {}) as Args) : h;
  });
  return mock;
}

/** A command that fails the way the backend reports errors. */
export const fails = (code: ErrorCode, message = "") => () => Promise.reject({ code, message });

/** The arguments of each call to one command, in order. */
export function calls(cmd: string): Args[] {
  return vi
    .mocked(invoke)
    .mock.calls.filter((c) => c[0] === cmd)
    .map((c) => (c[1] ?? {}) as Args);
}

export function resetTauri() {
  listeners.clear();
  answer({});
}
