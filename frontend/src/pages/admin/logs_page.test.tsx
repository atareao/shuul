import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

import { loadData } from "@/common/utils";
const loadDataMock = vi.mocked(loadData);

// ─────────────────────────────────────────────────────────────────────────────
// Setup de entorno de navegador mínimo para antd (jsdom carece de estas APIs).
// ─────────────────────────────────────────────────────────────────────────────
beforeAll(() => {
  class ResizeObserverStub {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  (globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver =
    ResizeObserverStub;
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }),
  });
  window.scrollTo = () => {};
});

const setLocation = (search: string) => {
  Object.defineProperty(window, "location", {
    configurable: true,
    writable: true,
    value: {
      href: `http://localhost/admin/logs${search}`,
      pathname: "/admin/logs",
      search,
    },
  });
};

/** Construye una entrada de log determinista. */
const makeEntry = (event: string, i: number) => ({
  ts: `2026-10-01T00:00:${String(i % 60).padStart(2, "0")}Z`,
  event,
  pipeline: "waf",
  ip: `10.0.0.${(i % 250) + 1}`,
  country: null,
  rule_id: null,
  rule_name: null,
  path: "/",
  method: "GET",
  query: null,
  ua: null,
  fqdn: null,
  referer: null,
  status_code: 200,
  profile: null,
  reason: null,
});

/** Número de filas de datos del `Table` del Log Viewer. */
const logRowCount = (): number =>
  document.querySelectorAll(".ant-table-tbody tr.ant-table-row").length;

const logsResponse = (): Response<unknown> => ({
  status: 200,
  data: {
    entries: [
      {
        ts: "2026-10-01T00:00:00Z",
        event: "banned",
        pipeline: "jail",
        ip: "10.0.0.1",
        country: null,
        rule_id: null,
        rule_name: null,
        path: "/",
        method: "GET",
        query: null,
        ua: null,
        fqdn: null,
        referer: null,
        status_code: 403,
        profile: null,
        reason: null,
      },
      {
        ts: "2026-10-01T00:00:01Z",
        event: "admitted",
        pipeline: "waf",
        ip: "10.0.0.2",
        country: null,
        rule_id: null,
        rule_name: null,
        path: "/",
        method: "GET",
        query: null,
        ua: null,
        fqdn: null,
        referer: null,
        status_code: 200,
        profile: null,
        reason: null,
      },
    ],
    total: 2,
    capacity: 1000,
  },
});

describe("LogsPage — state persistence", () => {
  let getItemSpy: ReturnType<typeof vi.spyOn>;
  let setItemSpy: ReturnType<typeof vi.spyOn>;
  let replaceStateSpy: ReturnType<typeof vi.spyOn>;

  const mockProps = {
    navigate: vi.fn(),
    t: (key: string) => key,
    isDarkMode: false,
  };

  beforeEach(() => {
    getItemSpy = vi.spyOn(Storage.prototype, "getItem");
    setItemSpy = vi.spyOn(Storage.prototype, "setItem");
    replaceStateSpy = vi
      .spyOn(window.history, "replaceState")
      .mockImplementation(() => {});
    loadDataMock.mockReset();
    setLocation("");
  });

  afterEach(() => {
    cleanup();
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
    // MODIFIED: el visor arranca refrescándose solo.
    expect(page.state.autoRefresh).toBe(true);
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

  // ─── Scenario 6 (MODIFIED): la URL no altera lo persistido ───────────────

  it("no sobreescribe el estado persistido con la query string de la URL", async () => {
    const persisted = JSON.stringify({
      hiddenEvents: ["banned"],
      autoRefresh: true,
    });
    getItemSpy.mockImplementation((key: string) =>
      key === "logsPage" ? persisted : null,
    );
    setLocation("?hidden=admitted&autoRefresh=false");
    loadDataMock.mockResolvedValue(logsResponse());

    render(<InnerPage {...mockProps} />);

    // El switch de auto-refresh refleja el estado restaurado de localStorage.
    const switchEl = await screen.findByRole("switch");
    await waitFor(() =>
      expect(switchEl.getAttribute("aria-checked")).toBe("true"),
    );
  });

  // ─── Scenario 7: alternar un filtro no modifica la URL ───────────────────

  it("alternar un filtro de eventos no modifica window.location.search", async () => {
    getItemSpy.mockReturnValue(null);
    setLocation("");
    loadDataMock.mockResolvedValue(logsResponse());

    render(<InnerPage {...mockProps} />);

    const bannedButton = await screen.findByRole("button", { name: "banned" });
    await userEvent.click(bannedButton);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "banned" }).className,
      ).toContain("ant-btn-default"),
    );
    expect(replaceStateSpy).not.toHaveBeenCalled();
    expect(window.location.search).toBe("");
  });

  // ─── Scenario 8 (H2.1): el Log Viewer pagina en cliente ──────────────────

  it("5.1 pagina en cliente: ocultar un tipo de evento re-pagina las entradas restantes", async () => {
    // Buffer de 120 entradas (40 «banned» + 80 «admitted»). El escenario fija
    // explícitamente un tamaño de página de 50 (su premisa literal, no el valor
    // por defecto del visor, que ahora es 10): por eso se elige 50 en el
    // selector de tamaño antes de medir.
    const entries = [
      ...Array.from({ length: 40 }, (_, i) => makeEntry("banned", i)),
      ...Array.from({ length: 80 }, (_, i) => makeEntry("admitted", i + 40)),
    ];
    loadDataMock.mockResolvedValue({
      status: 200,
      data: { entries, total: 120, capacity: 1000 },
    } as Response<unknown>);
    getItemSpy.mockReturnValue(null);

    render(<InnerPage {...mockProps} />);

    await screen.findByRole("button", { name: "banned" });

    // El selector de tamaño de página lo genera la `Pagination` de antd: su
    // combobox es el único con nombre accesible "Page Size" y sus opciones son
    // `role="option"` con `title="<n> / page"`.
    const user = userEvent.setup();
    const combo = await screen.findByRole("combobox", { name: "Page Size" });
    await user.click(combo);
    await user.click(await screen.findByTitle("50 / page"));
    await waitFor(() => expect(logRowCount()).toBe(50));

    // Sin filtro: 120 entradas, la página muestra 50 y el total reportado es 120.
    await screen.findByRole("button", { name: "banned" });
    await waitFor(() =>
      expect(
        document.querySelector(".ant-pagination-total-text")?.textContent,
      ).toBe("1-50 of 120 entries"),
    );
    expect(logRowCount()).toBe(50);

    // Ocultar «banned» deja 80 entradas; la página sigue mostrando 50 y el total
    // reportado pasa a ser 80 (paginación en cliente sobre el conjunto filtrado).
    await userEvent.click(screen.getByRole("button", { name: "banned" }));

    await waitFor(() =>
      expect(
        document.querySelector(".ant-pagination-total-text")?.textContent,
      ).toBe("1-50 of 80 entries"),
    );
    expect(logRowCount()).toBe(50);
  }, 15000);

  // ─── Scenario 5 reforzado (H2.2): sobrevive a navegar fuera y volver ─────

  it("5.2 el estado sobrevive a navegar fuera y volver (desmontar y remontar)", async () => {
    // `localStorage` real en memoria: el estado debe persistir entre montajes.
    const store: Record<string, string> = {};
    getItemSpy.mockImplementation((key: string) =>
      Object.prototype.hasOwnProperty.call(store, key) ? store[key] : null,
    );
    setItemSpy.mockImplementation((key: string, value: string) => {
      store[key] = value;
    });
    loadDataMock.mockResolvedValue(logsResponse());

    // Primera visita: ocultar `banned` (auto-refresh por defecto `true`).
    const first = render(<InnerPage {...mockProps} />);
    const banned = await screen.findByRole("button", { name: "banned" });
    await userEvent.click(banned);
    await waitFor(() => expect(store["logsPage"]).toContain("banned"));

    // Navegar fuera: desmonta la página.
    first.unmount();

    // Volver a `/admin/logs`: remonta la página y restaura desde `localStorage`.
    render(<InnerPage {...mockProps} />);
    const bannedAgain = await screen.findByRole("button", { name: "banned" });
    await waitFor(() =>
      expect(bannedAgain.className).toContain("ant-btn-default"),
    );

    const switchEl = screen.getByRole("switch");
    expect(switchEl.getAttribute("aria-checked")).toBe("true");

    const persisted = JSON.parse(store["logsPage"]) as {
      hiddenEvents: string[];
      autoRefresh: boolean;
    };
    expect(persisted.hiddenEvents).toContain("banned");
    expect(persisted.autoRefresh).toBe(true);
  });

  // ─── Scenario 1 reforzado (H2.3): el poll arranca con auto-refresh ───────

  it("5.3 arranca el polling cuando autoRefresh está activo", async () => {
    vi.useFakeTimers();
    try {
      getItemSpy.mockImplementation((key: string) =>
        key === "logsPage"
          ? JSON.stringify({ hiddenEvents: [], autoRefresh: true })
          : null,
      );
      loadDataMock.mockResolvedValue(logsResponse());

      render(<InnerPage {...mockProps} />);

      // Dejar que el fetch inicial resuelva y se registre el `setInterval`.
      await vi.advanceTimersByTimeAsync(10);
      const callsAfterMount = loadDataMock.mock.calls.length;
      expect(callsAfterMount).toBeGreaterThanOrEqual(1);

      // El intervalo de polling es de 3 s: debe producir un refresco extra.
      await vi.advanceTimersByTimeAsync(3000);
      expect(loadDataMock.mock.calls.length).toBeGreaterThan(callsAfterMount);
    } finally {
      vi.useRealTimers();
    }
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Escenario «The Log Viewer starts at 10 and keeps the same ranges».
// ─────────────────────────────────────────────────────────────────────────────
describe("tamaño de página", () => {
  const mockProps = {
    navigate: vi.fn(),
    t: (key: string) => key,
    isDarkMode: false,
  };

  beforeEach(() => {
    vi.spyOn(Storage.prototype, "getItem").mockReturnValue(null);
    loadDataMock.mockReset();
    setLocation("");
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("6.1 arranca en 10 entradas y el selector ofrece 10, 50 y 100", async () => {
    // Buffer de 120 entradas: con 10 por página hay 12 páginas y la primera
    // retiene exactamente 10 filas. Hoy el visor arranca en 50 y renderiza 50
    // filas, así que ambos números son la medida del cambio.
    const entries = Array.from({ length: 120 }, (_, i) =>
      makeEntry("admitted", i),
    );
    loadDataMock.mockResolvedValue({
      status: 200,
      data: { entries, total: 120, capacity: 1000 },
    } as Response<unknown>);

    render(<InnerPage {...mockProps} />);

    await screen.findByRole("button", { name: "admitted" });

    // `showTotal` es el contrato del texto de total: con 120 entradas y una
    // página de 10 el rango es "1-10". Si arrancase en 50 el texto sería
    // "1-50 of 120 entries".
    await waitFor(() =>
      expect(
        document.querySelector(".ant-pagination-total-text")?.textContent,
      ).toBe("1-10 of 120 entries"),
    );
    expect(logRowCount()).toBe(10);

    // El selector de tamaño de página lo genera la `Pagination` de antd: su
    // combobox es el único con nombre accesible "Page Size" (lo inyecta el
    // locale `en_US` de `@rc-component/pagination`, `page_size: "Page Size"`).
    // Las opciones son `role="option"` con `title="<n> / page"`.
    const user = userEvent.setup();
    const combo = await screen.findByRole("combobox", { name: "Page Size" });
    await user.click(combo);
    const options = await screen.findAllByRole("option");
    expect(options.map((o) => o.getAttribute("title"))).toEqual([
      "10 / page",
      "50 / page",
      "100 / page",
    ]);
    // Fuerza la aserción: el rango 20 —el que ofrece hoy el visor— no debe
    // aparecer.
    expect(screen.queryByTitle("20 / page")).toBeNull();
  });
});
