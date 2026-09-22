import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { InnerPage } from "./logs_page";
import type Response from "@/models/response";

// Mock loadData to avoid actual API calls
vi.mock("@/common/utils", () => ({
  loadData: vi.fn<() => Promise<Response<unknown>>>(),
}));

// Mock constants
vi.mock("@/constants", () => ({
  BASE_URL: "http://localhost:3000",
}));

describe("LogsPage — state persistence", () => {
  let getItemSpy: ReturnType<typeof vi.spyOn>;
  let setItemSpy: ReturnType<typeof vi.spyOn>;

  const mockProps = {
    navigate: vi.fn(),
    t: (key: string) => key,
    isDarkMode: false,
  };

  beforeEach(() => {
    getItemSpy = vi.spyOn(Storage.prototype, "getItem");
    setItemSpy = vi.spyOn(Storage.prototype, "setItem");
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // ─── Scenario 1: State restored from localStorage on mount ───────────────

  it("restores hiddenEvents and autoRefresh from localStorage when logsPage key exists", () => {
    const persisted = JSON.stringify({
      hiddenEvents: ["banned"],
      autoRefresh: true,
    });
    getItemSpy.mockImplementation((key: string) => {
      if (key === "logsPage") return persisted;
      return null;
    });

    const page = new InnerPage(mockProps);

    expect(page.state.hiddenEvents).toEqual(["banned"]);
    expect(page.state.autoRefresh).toBe(true);
  });

  // ─── Scenario 2: State defaults when localStorage is empty ───────────────

  it("uses default state when localStorage has no logsPage key", () => {
    getItemSpy.mockReturnValue(null);

    const page = new InnerPage(mockProps);

    expect(page.state.hiddenEvents).toEqual([]);
    expect(page.state.autoRefresh).toBe(false);
  });

  // ─── Scenario 3: Toggling event filter persists immediately ─────────────

  it("persists hiddenEvents to localStorage after toggleEventFilter", () => {
    getItemSpy.mockReturnValue(null);
    const page = new InnerPage(mockProps);

    page.toggleEventFilter("banned");

    // setItem should have been called with logsPage key containing banned
    const setItemCall = setItemSpy.mock.calls.find(
      (call: [string, string]) => call[0] === "logsPage",
    );
    expect(setItemCall).toBeDefined();
    const stored = JSON.parse(setItemCall![1] as string);
    expect(stored.hiddenEvents).toContain("banned");
  });

  // ─── Scenario 4: Toggling auto-refresh persists immediately ─────────────

  it("persists autoRefresh to localStorage after toggleAutoRefresh", () => {
    getItemSpy.mockReturnValue(null);
    const page = new InnerPage(mockProps);

    page.toggleAutoRefresh(true);

    const setItemCall = setItemSpy.mock.calls.find(
      (call: [string, string]) => call[0] === "logsPage",
    );
    expect(setItemCall).toBeDefined();
    const stored = JSON.parse(setItemCall![1] as string);
    expect(stored.autoRefresh).toBe(true);
  });

  // ─── Scenario 5: State survives navigation away ─────────────────────────

  it("writes current state to localStorage on componentWillUnmount", () => {
    getItemSpy.mockReturnValue(null);
    const page = new InnerPage(mockProps);

    // Mutate state
    page.toggleEventFilter("block");
    page.toggleAutoRefresh(true);

    // Clear the setItem calls from the mutations so we can check the unmount call
    setItemSpy.mockClear();

    page.componentWillUnmount();

    const setItemCall = setItemSpy.mock.calls.find(
      (call: [string, string]) => call[0] === "logsPage",
    );
    expect(setItemCall).toBeDefined();
    const stored = JSON.parse(setItemCall![1] as string);
    expect(stored.hiddenEvents).toContain("block");
    expect(stored.autoRefresh).toBe(true);
  });
});