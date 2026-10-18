// jsdom has <audio> elements but plays nothing. This makes them act as if they played: play and
// pause fire their events, and a test moves the sound along or ends it.

import { act } from "@testing-library/react";
import { vi } from "vitest";

type Media = { paused: boolean; time: number };

export function fakeAudio() {
  const players: HTMLMediaElement[] = [];
  const state = new WeakMap<HTMLMediaElement, Media>();
  const of = (el: HTMLMediaElement) => {
    let s = state.get(el);
    if (!s) state.set(el, (s = { paused: true, time: 0 }));
    return s;
  };
  const proto = HTMLMediaElement.prototype;
  vi.spyOn(proto, "paused", "get").mockImplementation(function (this: HTMLMediaElement) {
    return of(this).paused;
  });
  vi.spyOn(proto, "currentTime", "get").mockImplementation(function (this: HTMLMediaElement) {
    return of(this).time;
  });
  vi.spyOn(proto, "currentTime", "set").mockImplementation(function (this: HTMLMediaElement, v: number) {
    of(this).time = v;
  });
  vi.spyOn(proto, "src", "set").mockImplementation(function (this: HTMLMediaElement, v: string) {
    this.setAttribute("src", v);
    if (!players.includes(this)) players.push(this);
  });
  vi.spyOn(proto, "play").mockImplementation(function (this: HTMLMediaElement) {
    of(this).paused = false;
    this.dispatchEvent(new Event("play"));
    return Promise.resolve();
  });
  vi.spyOn(proto, "pause").mockImplementation(function (this: HTMLMediaElement) {
    if (of(this).paused) return;
    of(this).paused = true;
    this.dispatchEvent(new Event("pause"));
  });
  vi.spyOn(proto, "load").mockImplementation(() => {});

  const player = () => {
    const p = players.at(-1);
    if (!p) throw new Error("nothing has been played");
    return p;
  };
  const fire = (type: string) => act(() => void player().dispatchEvent(new Event(type)));
  return {
    /** The file being played, as its URL. */
    get src() {
      return player().getAttribute("src");
    },
    get seconds() {
      return of(player()).time;
    },
    get playing() {
      return !of(player()).paused;
    },
    /** The file has loaded far enough to play. */
    loaded: () => fire("loadedmetadata"),
    /** The sound has played to `seconds` into the file. */
    at(seconds: number) {
      of(player()).time = seconds;
      fire("timeupdate");
    },
    end() {
      of(player()).paused = true;
      fire("ended");
    },
    fail: () => fire("error"),
  };
}
