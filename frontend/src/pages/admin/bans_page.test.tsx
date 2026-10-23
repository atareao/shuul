import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from "vitest";
import { render, waitFor, cleanup } from "@testing-library/react";
import { InnerPage } from "./bans_page";
import type Ban from "@/models/ban";
import type Response from "@/models/response";

// La página renderiza `CustomTable`, que además de `loadData` usa `debounce`,
// `mapsEqual` y `getNestedValue`: se sustituye únicamente la llamada a la API.
vi.mock("@/common/utils", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/common/utils")>();
  return { ...actual, loadData: vi.fn() };
});

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

/** Fila de datos del `Table` del listado de baneos. */
const rowCount = (): number =>
  document.querySelectorAll(".ant-table-tbody tr.ant-table-row").length;

/** Claves de fila que antd publica en el DOM. */
const dataRowKeys = (): (string | null)[] =>
  Array.from(
    document.querySelectorAll(".ant-table-tbody tr.ant-table-row"),
  ).map((tr) => tr.getAttribute("data-row-key"));

/** Claves de las filas cuyo contenido menciona `text`. */
const keysOfRowsContaining = (text: string): (string | null)[] =>
  Array.from(
    document.querySelectorAll(".ant-table-tbody tr.ant-table-row"),
  )
    .filter((tr) => (tr.textContent ?? "").includes(text))
    .map((tr) => tr.getAttribute("data-row-key"));

/** Texto literal con el que React denuncia dos hijos con la misma `key`. */
const DUPLICATE_KEY_WARNING = /Encountered two children with the same key/;

/** Baneo servido por la API: `id` es la IP, `rule_id` la partición que distingue. */
const makeBan = (ip: string, ruleId: number | undefined, i: number): Ban => ({
  id: ip,
  ip_address: ip,
  rule_id: ruleId,
  rule_name: ruleId === undefined ? "Unmatched" : `Rule ${ruleId}`,
  reason: `reason-${i}`,
  ban_duration_seconds: 900,
  escalation_level: 0,
  time_remaining_seconds: 600,
});

/**
 * Página de 10 baneos en la que la 5.ª fila repite la IP de la 3.ª: la misma IP
 * baneada por dos reglas distintas (`rule_id` 8 y 6). Como `id` ES la IP, esas
 * dos filas comparten `id`: es la condición que repetía la clave de fila.
 */
const duplicateIpBans = (): Ban[] =>
  Array.from({ length: 10 }, (_, i) => {
    const duplicate = i === 4;
    const ip = `10.0.0.${duplicate ? i - 1 : i + 1}`;
    return makeBan(ip, duplicate ? 6 : 8, i + 1);
  });

const bansResponse = (bans: Ban[]): Response<Ban[]> => ({
  status: 200,
  data: bans,
  pagination: { page: 1, limit: 10, pages: 1, records: bans.length },
});

const mockProps = {
  navigate: vi.fn(),
  t: (key: string) => key,
};

const renderPage = () => render(<InnerPage {...mockProps} />);

describe("BansPage — identidad de fila", () => {
  let consoleErrors: string[];

  beforeEach(() => {
    consoleErrors = [];
    vi.spyOn(console, "error").mockImplementation((...args: unknown[]) => {
      consoleErrors.push(args.map((arg) => String(arg)).join(" "));
    });
    loadDataMock.mockReset();
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  const duplicateKeyWarnings = (): string[] =>
    consoleErrors.filter((message) => DUPLICATE_KEY_WARNING.test(message));

  it("una página con dos baneos de la misma IP renderiza exactamente 10 filas y no provoca el aviso de claves repetidas", async () => {
    loadDataMock.mockResolvedValue(bansResponse(duplicateIpBans()));

    renderPage();
    await waitFor(() => expect(rowCount()).toBe(10));

    // Exactamente las 10 filas de la página: ni una más.
    expect(rowCount()).toBe(10);
    // Y React no ha registrado el aviso de claves duplicadas.
    expect(duplicateKeyWarnings()).toEqual([]);
  });

  it("los dos baneos de la misma IP reciben claves de fila distintas", async () => {
    loadDataMock.mockResolvedValue(bansResponse(duplicateIpBans()));

    renderPage();
    await waitFor(() => expect(rowCount()).toBe(10));

    // Las dos filas de la IP repetida: mismo `id`, particiones distintas.
    const repeatedIpKeys = keysOfRowsContaining("10.0.0.3");
    expect(repeatedIpKeys).toHaveLength(2);
    expect(repeatedIpKeys[0]).not.toBe(repeatedIpKeys[1]);

    // Todas las claves de la página son distintas.
    const keys = dataRowKeys();
    expect(keys).toHaveLength(10);
    expect(new Set(keys).size).toBe(10);
  });
});
