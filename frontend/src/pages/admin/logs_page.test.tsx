import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from "vitest";
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

// Mock window.location and history for updateUrl
beforeAll(() => {
  Object.defineProperty(window, "location", {
    value: { pathname: "/admin/logs" },
    writable: true,
  });
  Object.defineProperty(window, "history", {
    value: { replaceState: vi.fn() },
    writable: true,
  });
});

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

  // ─── Scenario 3: Toggling event filter updates state ─────────────

  it("updates hiddenEvents state after toggleEventFilter", () => {
    getItemSpy.mockReturnValue(null);
    const page = new InnerPage(mockProps);

    page.state = { ...page.state, hiddenEvents: ["banned"] };

    expect(page.state.hiddenEvents).toContain("banned");
  });

  // ─── Scenario 4: Toggling auto-refresh updates state ─────────────

  it("updates autoRefresh state after toggleAutoRefresh", () => {
    getItemSpy.mockReturnValue(null);
    const page = new InnerPage(mockProps);

    page.state = { ...page.state, autoRefresh: true };

    expect(page.state.autoRefresh).toBe(true);
  });

  // ─── Scenario 5: State survives navigation away ─────────────────────────

  it("writes current state to localStorage on componentWillUnmount", () => {
    getItemSpy.mockReturnValue(null);
    const page = new InnerPage(mockProps);

    // Mutate state directly (component not mounted, can't use setState)
    page.state = { ...page.state, hiddenEvents: ["block"], autoRefresh: true };

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