import { beforeEach, describe, expect, it } from "vitest";
import { lineHistory } from "./line";

describe("live line history", () => {
  beforeEach(() => {
    lineHistory.start(`reset-${Math.random()}`);
  });

  it("keeps its line for the same recording and starts empty for a new one", () => {
    lineHistory.start("s1");
    lineHistory.push({ app: -10 });
    lineHistory.start("s1");
    expect(lineHistory.samples).toHaveLength(1);
    lineHistory.start("s2");
    expect(lineHistory.samples).toHaveLength(0);
  });

  it("draws 相手 from the louder of app and system, and 自分 from the mic", () => {
    lineHistory.push({ app: -60, system: -6 });
    const [s] = lineHistory.samples;
    expect(s.o).toBeGreaterThan(0);
    expect(s.m).toBe(0);
    lineHistory.push({ mic: -6 });
    expect(lineHistory.samples[1].m).toBeGreaterThan(0);
    // 相手 falls back slowly instead of dropping to nothing.
    expect(lineHistory.samples[1].o).toBeGreaterThan(0);
    expect(lineHistory.samples[1].o).toBeLessThan(s.o);
  });

  it("marks a screenshot on the newest step", () => {
    lineHistory.shot();
    expect(lineHistory.samples).toHaveLength(0);
    lineHistory.push({});
    lineHistory.push({});
    lineHistory.shot();
    expect(lineHistory.samples.map((s) => !!s.shot)).toEqual([false, true]);
  });

  it("leaves a flat gap after a pause", () => {
    lineHistory.push({ app: -6 });
    lineHistory.gap();
    expect(lineHistory.samples.slice(1).every((s) => s.gap && s.o === 0)).toBe(true);
    expect(lineHistory.samples).toHaveLength(9);
    lineHistory.push({});
    expect(lineHistory.samples.at(-1)?.o).toBe(0);
  });

  it("keeps ten minutes of steps", () => {
    for (let i = 0; i < 6010; i++) lineHistory.push({});
    expect(lineHistory.samples).toHaveLength(6000);
  });
});
