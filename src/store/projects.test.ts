import { beforeEach, describe, expect, it } from "vitest";
import { answer, calls } from "../test/tauri";
import { resetStores } from "../test/fixtures";
import { projectOf, useProjects } from "./projects";

const work = { id: "p1", name: "仕事", color: "red" };
const home = { id: "p2", name: "家", color: "blue" };

describe("projects store", () => {
  beforeEach(resetStores);

  it("adds a new project, or replaces one with the same ID", async () => {
    answer({ create_project: ({ name }: { name: string }) => ({ ...work, name }) });
    useProjects.setState({ projects: [work, home] });
    await useProjects.getState().create("会議", "red");
    expect(useProjects.getState().projects.map((p) => p.name)).toEqual(["家", "会議"]);
  });

  it("reloads lists of sessions when a project is deleted or sessions move", async () => {
    answer({ delete_project: [home] });
    useProjects.setState({ projects: [work, home] });
    await useProjects.getState().remove("p1");
    expect(useProjects.getState().projects).toEqual([home]);
    expect(useProjects.getState().version).toBe(1);
    await useProjects.getState().assign(["s1", "s2"], null);
    expect(calls("set_project")).toEqual([{ sessionIds: ["s1", "s2"], project: null }]);
    expect(useProjects.getState().version).toBe(2);
  });

  it("finds a session's project only while it exists", () => {
    expect(projectOf([work], "p1")).toBe(work);
    expect(projectOf([work], "gone")).toBeUndefined();
    expect(projectOf([work], null)).toBeUndefined();
  });
});
