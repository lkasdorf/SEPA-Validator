import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import type { ValidationEvent, ValidationResult } from "./types";

type OnEvent = (ev: ValidationEvent) => void;
const listeners: OnEvent[] = [];
const startValidation = vi.fn((_paths: string[], onEvent: OnEvent) => {
  listeners.push(onEvent);
  return Promise.resolve();
});
const cancelValidationApi = vi.fn(() => Promise.resolve());
vi.mock("./api", () => ({
  startValidation: (p: string[], cb: OnEvent) => startValidation(p, cb),
  cancelValidation: () => cancelValidationApi(),
}));

const { runValidation, cancelValidation } = await import("./validation");
const { results, progress, runId } = await import("./stores");

const result = (file: string): ValidationResult => ({
  file,
  path: file,
  namespace: "",
  schema: "",
  status: "ok",
  errors: 0,
  warnings: 0,
  messages: [],
});
const emitResult = (run: number, index: number, file: string) =>
  listeners[run]({ event: "result", data: { index, result: result(file) } });

describe("runValidation", () => {
  beforeEach(() => {
    listeners.length = 0;
    startValidation.mockClear();
    cancelValidationApi.mockClear();
  });

  it("ignores events from a run that was superseded by a newer one", async () => {
    await runValidation(["a.xml"]);
    await runValidation(["b.xml"]);
    emitResult(0, 0, "a.xml");
    emitResult(1, 0, "b.xml");
    expect(get(results).map((r) => r.file)).toEqual(["b.xml"]);
  });

  it("gives every run a new id", async () => {
    await runValidation(["a.xml"]);
    const first = get(runId);
    await runValidation(["a.xml"]);
    expect(get(runId)).toBeGreaterThan(first);
  });

  it("stops the run in the UI immediately on cancel and ignores later events", async () => {
    await runValidation(["a.xml", "b.xml"]);
    listeners[0]({ event: "started", data: { total: 2 } });
    emitResult(0, 0, "a.xml");
    await cancelValidation();
    emitResult(0, 1, "b.xml");
    expect(cancelValidationApi).toHaveBeenCalledOnce();
    expect(get(progress)).toMatchObject({ running: false, cancelled: true, done: 1, total: 2 });
    expect(get(results).map((r) => r.file)).toEqual(["a.xml"]);
  });

  it("marks a run cancelled when the backend reports it stopped early", async () => {
    await runValidation(["a.xml"]);
    listeners[0]({ event: "finished", data: { total: 1, cancelled: true } });
    expect(get(progress)).toMatchObject({ running: false, cancelled: true });
  });

  it("stops showing progress when the run could not be started", async () => {
    startValidation.mockImplementationOnce(() => Promise.reject(new Error("ipc down")));
    await runValidation(["a.xml"]);
    expect(get(progress).running).toBe(false);
  });
});
