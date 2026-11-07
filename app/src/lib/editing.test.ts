import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import type { EditedValidation, ValidationResult } from "./types";

const validateEdited = vi.fn<(path: string, text: string) => Promise<EditedValidation>>();
const saveEdited = vi.fn<(source: string, text: string) => Promise<ValidationResult>>();
vi.mock("./api", () => ({
  validateEdited: (p: string, t: string) => validateEdited(p, t),
  saveEdited: (s: string, t: string) => saveEdited(s, t),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: () => Promise.resolve(true) }));

const { registerEditor, validateDraft, saveDraft, discardDraft, editNote } = await import("./editing");
const { drafts, setDirty, clearDrafts, shownResult } = await import("./drafts");
const { results, selectedIndex, selectedResult } = await import("./stores");

const A = String.raw`C:\d\a.xml`;
const result = (path: string, status: ValidationResult["status"], line?: number): ValidationResult => ({
  file: path.split("\\").pop()!, path, namespace: "", schema: "", status, errors: 0, warnings: 0,
  messages: line ? [{ severity: "error", text: "x", line, column: null, hint: null }] : [],
});

// A fake editor: its text, with dirty tracking like the CodeViewer's update listener.
const ORIGINAL = "<a>\n  <b>1</b>\n</a>";
let text = ORIGINAL;
function type(t: string) {
  text = t;
  setDirty(get(selectedResult)!.path, text !== ORIGINAL);
}

describe("editing", () => {
  beforeEach(() => {
    clearDrafts();
    validateEdited.mockReset();
    saveEdited.mockReset();
    results.set([result(A, "invalid", 2), result(String.raw`C:\d\b.xml`, "ok")]);
    selectedIndex.set(0);
    text = ORIGINAL;
    registerEditor({
      path: () => get(selectedResult)?.path ?? "",
      text: () => text,
      setText: (t) => type(t),
      revert: () => type(ORIGINAL),
    });
  });

  it("validates the edited text, shows it re-indented and keeps the result as the draft's", async () => {
    type("<a><b>2</b></a>");
    validateEdited.mockResolvedValue({ formatted: "<a>\n  <b>2</b>\n</a>", result: result(A, "ok") });
    await validateDraft();
    expect(validateEdited).toHaveBeenCalledWith(A, "<a><b>2</b></a>");
    expect(text).toBe("<a>\n  <b>2</b>\n</a>");
    expect(get(drafts)[A]).toMatchObject({ stale: false, result: { status: "ok" } });
    expect(get(shownResult)?.status).toBe("ok");
  });

  it("keeps malformed text as typed (its error lines refer to it)", async () => {
    type("<a><b>2</a>");
    validateEdited.mockResolvedValue({ formatted: null, result: result(A, "error", 1) });
    await validateDraft();
    expect(text).toBe("<a><b>2</a>");
    expect(get(shownResult)?.status).toBe("error");
  });

  it("drops the result when the text changed while validating", async () => {
    type("<a><b>2</b></a>");
    validateEdited.mockImplementation(async () => {
      type("<a><b>3</b></a>");
      return { formatted: "<a>\n  <b>2</b>\n</a>", result: result(A, "ok") };
    });
    await validateDraft();
    expect(text).toBe("<a><b>3</b></a>");
    expect(get(drafts)[A].result).toBeNull();
    expect(get(editNote)?.failed).toBe(true);
  });

  it("does nothing without edits", async () => {
    await validateDraft();
    await saveDraft();
    expect(validateEdited).not.toHaveBeenCalled();
    expect(saveEdited).not.toHaveBeenCalled();
  });

  it("saves a copy, lists it right after the original and selects it", async () => {
    type("<a><b>2</b></a>");
    saveEdited.mockResolvedValue(result(String.raw`C:\d\a_edit1.xml`, "ok"));
    await saveDraft();
    expect(saveEdited).toHaveBeenCalledWith(A, "<a><b>2</b></a>");
    expect(get(results).map((r) => r.file)).toEqual(["a.xml", "a_edit1.xml", "b.xml"]);
    expect(get(selectedResult)?.file).toBe("a_edit1.xml");
    expect(get(drafts)).toEqual({});
  });

  it("keeps the draft when saving fails", async () => {
    type("<a><b>2</b></a>");
    saveEdited.mockRejectedValue("Refusing to write");
    await saveDraft();
    expect(get(results)).toHaveLength(2);
    expect(get(drafts)[A]).toBeTruthy();
    expect(get(editNote)).toEqual({ text: "Refusing to write", failed: true });
  });

  it("discard puts the file's text back", () => {
    type("<a><b>2</b></a>");
    discardDraft();
    expect(text).toBe(ORIGINAL);
    expect(get(drafts)).toEqual({});
  });
});
