import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import type { ValidationResult } from "./types";

const ask = vi.fn(() => Promise.resolve(true));
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: () => ask() }));

const { drafts, setDirty, setDraftResult, shownResult, selectedDraft, clearDrafts, confirmDiscardDrafts, hasDrafts } =
  await import("./drafts");
const { results, selectedIndex } = await import("./stores");

const result = (path: string, status: ValidationResult["status"]): ValidationResult => ({
  file: path, path, namespace: "", schema: "", status, errors: 0, warnings: 0, messages: [],
});

describe("drafts", () => {
  beforeEach(() => {
    clearDrafts();
    ask.mockClear();
    results.set([result("a.xml", "invalid"), result("b.xml", "ok")]);
    selectedIndex.set(0);
  });

  it("tracks a file while its text differs and forgets it when equal again", () => {
    setDirty("a.xml", true);
    expect(get(selectedDraft)).toEqual({ result: null, stale: false });
    setDirty("a.xml", false);
    expect(get(drafts)).toEqual({});
    expect(hasDrafts()).toBe(false);
  });

  it("shows the edited text's result once validated, and marks it stale on further edits", () => {
    setDirty("a.xml", true);
    expect(get(shownResult)?.status).toBe("invalid"); // not validated yet: the file's result
    setDraftResult("a.xml", result("a.xml", "ok"));
    expect(get(shownResult)?.status).toBe("ok");
    setDirty("a.xml", true);
    expect(get(selectedDraft)?.stale).toBe(true);
    expect(get(shownResult)?.status).toBe("ok");
    selectedIndex.set(1);
    expect(get(selectedDraft)).toBeNull();
    expect(get(shownResult)?.path).toBe("b.xml");
  });

  it("ignores a result for a file without edits", () => {
    setDraftResult("b.xml", result("b.xml", "invalid"));
    expect(get(drafts)).toEqual({});
  });

  it("asks before discarding edits, and only when there are some", async () => {
    expect(await confirmDiscardDrafts("Close")).toBe(true);
    expect(ask).not.toHaveBeenCalled();
    setDirty("a.xml", true);
    ask.mockResolvedValueOnce(false);
    expect(await confirmDiscardDrafts("Close")).toBe(false);
    expect(ask).toHaveBeenCalledOnce();
  });
});
