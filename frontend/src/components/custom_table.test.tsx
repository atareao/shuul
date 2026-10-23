import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from "vitest";
import { render, waitFor, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CustomTable from "./custom_table";
import type { FieldDefinition } from "@/common/types";
import type Response from "@/models/response";
import type { QueryParams } from "@/common/utils";

// ─────────────────────────────────────────────────────────────────────────────
// Setup de entorno de navegador mínimo para antd (jsdom carece de estas APIs).
// Los polyfills viven en el test para no tocar la configuración compartida.
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

// Mock parcial de `loadData`: solo se sustituye la llamada a la API, el resto de
// utilidades (`debounce`, `mapsEqual`, `getNestedValue`) se mantienen reales.
vi.mock("@/common/utils", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/common/utils")>();
  return { ...actual, loadData: vi.fn() };
});

import { loadData } from "@/common/utils";
const loadDataMock = vi.mocked(loadData);

interface Row {
  id: number;
  ip_address: string;
  reason: string;
}

const FIELDS: FieldDefinition<Row>[] = [
  { key: "ip_address", label: "IP", type: "string", sortKey: "ip_address" },
  { key: "reason", label: "Reason", type: "string", sortKey: "reason" },
];

/** Dataset determinista de `records` filas. */
const dataset = (records: number): Row[] =>
  Array.from({ length: records }, (_, i) => ({
    id: i + 1,
    ip_address: `10.0.0.${i + 1}`,
    reason: `reason-${i + 1}`,
  }));

/**
 * Servidor de paginación en servidor. `rowsOverride` permite simular un servidor
 * que devuelve más filas que el `limit` pedido; `limitOverride` simula un
 * servidor que *reporta* un `limit` distinto al solicitado.
 */
const server =
  (
    records: number,
    opts: { rowsOverride?: number; limitOverride?: number } = {},
  ) =>
  (
    _endpoint: string,
    params?: QueryParams,
  ): Promise<Response<Row[]>> => {
    const page = Number(params?.get("page") ?? 1);
    const requestedLimit = Number(params?.get("limit") ?? 10);
    const reportedLimit = opts.limitOverride ?? requestedLimit;
    const rows = opts.rowsOverride ?? requestedLimit;
    const offset = (page - 1) * requestedLimit;
    return Promise.resolve({
      status: 200,
      data: dataset(records).slice(offset, offset + rows),
      pagination: {
        page,
        limit: reportedLimit,
        pages: Math.ceil(records / reportedLimit),
        records,
      },
    } as Response<Row[]>);
  };

const rowCount = (): number =>
  document.querySelectorAll(".ant-table-tbody tr.ant-table-row").length;

const lastParams = (): QueryParams | undefined => {
  const calls = loadDataMock.mock.calls;
  return calls[calls.length - 1]?.[1];
};

const sizeLabel = (): string | null | undefined =>
  document
    .querySelector(".ant-pagination-options-size-changer .ant-select-content")
    ?.getAttribute("title");

const totalLabel = (): string | null | undefined =>
  document.querySelector(".ant-pagination-total-text")?.textContent;

const nextButton = (): Element => {
  const el = document.querySelector(".ant-pagination-next");
  if (!el) throw new Error("no hay control de paginación 'next'");
  return el;
};

const renderTable = () =>
  render(
    <CustomTable<Row>
      title="Active Bans"
      endpoint="bans"
      fields={FIELDS}
      t={(key) => key}
    />,
  );

describe("CustomTable — paginación en servidor", () => {
  beforeEach(() => {
    loadDataMock.mockReset();
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  // ── 2.1 ────────────────────────────────────────────────────────────────────
  it("2.1 muestra exactamente las filas de la página pedida (10,10,10,4 y total 34/4)", async () => {
    loadDataMock.mockImplementation(server(34));

    renderTable();
    await waitFor(() => expect(rowCount()).toBe(10));

    // El total y las páginas proceden de la respuesta del servidor.
    expect(totalLabel()).toBe("Total: 34 records (4 pages)");
    expect(sizeLabel()).toBe("10 / page");

    await userEvent.click(nextButton());
    await waitFor(() => expect(lastParams()?.get("page")).toBe("2"));
    await waitFor(() => expect(rowCount()).toBe(10));

    await userEvent.click(nextButton());
    await waitFor(() => expect(lastParams()?.get("page")).toBe("3"));
    await waitFor(() => expect(rowCount()).toBe(10));

    await userEvent.click(nextButton());
    await waitFor(() => expect(lastParams()?.get("page")).toBe("4"));
    await waitFor(() => expect(rowCount()).toBe(4));
  });

  // ── 2.2 ────────────────────────────────────────────────────────────────────
  it("2.2 no deja que antd recorte una página ya paginada: el número de filas no cambia al pasar de página", async () => {
    // El servidor entrega una página de 12 filas mientras reporta limit=10: es el
    // escenario en el que antd vuelve a recortar un conjunto ya paginado
    // (dataSource.length > pageSize && dataSource.length < total).
    loadDataMock.mockImplementation(server(34, { rowsOverride: 12 }));

    renderTable();
    await waitFor(() => expect(rowCount()).toBeGreaterThan(0));
    const rowsOnFirstPage = rowCount();

    await userEvent.click(nextButton());
    await waitFor(() => expect(lastParams()?.get("page")).toBe("2"));
    const rowsOnSecondPage = rowCount();

    // SÍNTOMA REPORTADO: «muestra 10 filas y al pasar a la siguiente el número cambia».
    expect(rowsOnSecondPage).toBe(rowsOnFirstPage);
  });

  // ── 2.3 ────────────────────────────────────────────────────────────────────
  it("2.3 cambiar el tamaño de página a 50 no lo deja reescrito por el servidor y vuelve a la primera página", async () => {
    // Dataset deliberadamente grande (`records > pageSize * current`): con 340
    // registros y el tamaño 10, situarse en la página 10. El `onChange` de
    // rc-pagination conservaría una página intermedia (calcula `ceil(340/50)=7` y
    // como `10 > 7` mantiene `current`, no la primera); un dataset de 34 registros
    // haría pasar el test por casualidad, porque `ceil(34/50)=1`.
    // El servidor reporta siempre `limit=10` aunque se le pida 50 (el pageSize es
    // propiedad del cliente y no debe reescribirse con el valor de la respuesta).
    loadDataMock.mockImplementation(server(340, { limitOverride: 10 }));

    renderTable();
    await waitFor(() => expect(rowCount()).toBe(10));
    expect(totalLabel()).toBe("Total: 340 records (34 pages)");

    // Situarse en la página 10 antes de cambiar el tamaño.
    for (let p = 2; p <= 10; p++) {
      await userEvent.click(nextButton());
      await waitFor(() => expect(lastParams()?.get("page")).toBe(String(p)));
    }
    expect(rowCount()).toBe(10);

    // Abrir el selector de tamaño y elegir 50.
    const changer = document.querySelector(
      ".ant-pagination-options-size-changer",
    );
    await userEvent.click(changer as Element);
    await userEvent.click(await screen.findByText("50 / page"));

    // Cambiar el tamaño vuelve a la PRIMERA página y pide limit=50.
    await waitFor(() => expect(lastParams()?.get("limit")).toBe("50"));
    expect(lastParams()?.get("page")).toBe("1");

    // La tabla muestra la primera página de 50 registros.
    await waitFor(() => expect(rowCount()).toBe(50));

    // El pageSize del cliente no se reescribe con el limit de la respuesta.
    await waitFor(() => expect(sizeLabel()).toBe("50 / page"));
  });

  // ── 3.1 ────────────────────────────────────────────────────────────────────
  it("3.1 no muestra la barra de paginación cuando el total es 0", async () => {
    // Guarda de regresión (H3): antd no renderizaba paginación en una tabla vacía;
    // el `<Pagination>` externo no debe introducir ese ruido visual con total 0.
    loadDataMock.mockImplementation(server(0));

    renderTable();
    // Esperar a que termine la carga inicial.
    await waitFor(() =>
      expect(document.querySelector(".ant-spin-spinning")).toBeNull(),
    );

    expect(document.querySelector(".ant-pagination")).toBeNull();
    expect(totalLabel()).toBeUndefined();
  });

  // ── 3.2 ────────────────────────────────────────────────────────────────────
  it("3.2 la rama de error resetea el total: una lista vacía no muestra las páginas anteriores", async () => {
    // Guarda de regresión (H4): una primera respuesta de 34 registros deja el
    // total en 34; si la siguiente falla, el total no debe quedar obsoleto.
    loadDataMock.mockResolvedValue({
      status: 200,
      data: dataset(34).slice(0, 10),
      pagination: { page: 1, limit: 10, pages: 4, records: 34 },
    } as Response<Row[]>);

    const { rerender } = render(
      <CustomTable<Row>
        title="Active Bans"
        endpoint="bans"
        fields={FIELDS}
        t={(key) => key}
        params={new Map([["probe", "a"]])}
      />,
    );
    await waitFor(() => expect(rowCount()).toBe(10));
    expect(totalLabel()).toBe("Total: 34 records (4 pages)");

    // La siguiente respuesta es un error.
    loadDataMock.mockResolvedValue({ status: 500, message: "boom" });

    // Un cambio de `params` dispara un nuevo `fetchData`.
    rerender(
      <CustomTable<Row>
        title="Active Bans"
        endpoint="bans"
        fields={FIELDS}
        t={(key) => key}
        params={new Map([["probe", "b"]])}
      />,
    );

    await waitFor(() => expect(rowCount()).toBe(0));
    // El total quedó a 0 → no se muestran las páginas de la respuesta anterior.
    expect(document.querySelector(".ant-pagination")).toBeNull();
    expect(totalLabel()).toBeUndefined();
  });

  // ── 2.4 ────────────────────────────────────────────────────────────────────
  it("2.4 la ordenación descendente envía asc=false junto a sort_by y vuelve a la primera página", async () => {
    loadDataMock.mockImplementation(server(34));

    const { container } = renderTable();
    await waitFor(() => expect(rowCount()).toBe(10));

    // Avanzar a la segunda página para comprobar que ordenar por una columna nueva
    // reinicia a la primera página.
    await userEvent.click(nextButton());
    await waitFor(() => expect(lastParams()?.get("page")).toBe("2"));

    const headers = container.querySelectorAll(".ant-table-thead th");
    const ipHeader = headers[0] as Element;

    // Dos clics: ascendente y luego descendente.
    await userEvent.click(ipHeader);
    await waitFor(() => expect(lastParams()?.get("asc")).toBe("true"));
    await userEvent.click(ipHeader);
    await waitFor(() => expect(lastParams()?.get("asc")).toBe("false"));

    const params = lastParams();
    expect(params?.get("sort_by")).toBe("ip_address");
    expect(params?.get("asc")).toBe("false");
    expect(params?.get("page")).toBe("1");
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Identidad de fila.
//
// Los baneos se sirven con una fila por partición `(ip_address, rule_id)`, así
// que una misma IP puede aparecer en dos filas y el `id` —que es la IP— deja de
// ser único dentro de la página. `CustomTable` usaba `record.id` como clave de
// fila: con dos hijos con la misma `key` React no puede reconciliar el `<tbody>`,
// avisa por consola y la tabla ACUMULA filas al navegar (medido: una página de 10
// filas con una IP repetida pasa a 11 y a 12 al ir y volver). La tabla debe
// poder recibir la partición completa como clave.
// ─────────────────────────────────────────────────────────────────────────────

/** Baneo: `id` es la IP y `rule_id` la partición que distingue dos filas. */
interface BanRow {
  id: string;
  ip_address: string;
  rule_id?: number;
}

/** Clave de fila = partición completa `(ip_address, rule_id)`. */
const banRowKey = (ban: BanRow): string =>
  `${ban.ip_address}#${ban.rule_id ?? "none"}`;

const BAN_FIELDS: FieldDefinition<BanRow>[] = [
  { key: "ip_address", label: "IP", type: "string", sortKey: "ip_address" },
];

/**
 * Página de 10 filas donde la 5.ª repite la IP de la 3.ª: la misma IP baneada
 * por dos reglas distintas (`rule_id` 8 y 6). En el listado de baneos el `id` de
 * esas dos filas es la misma IP.
 */
const duplicateIpPage = (): BanRow[] =>
  Array.from({ length: 10 }, (_, i) => {
    const duplicate = i === 4;
    const ip = `10.0.0.${duplicate ? i - 1 : i + 1}`;
    return { id: ip, ip_address: ip, rule_id: duplicate ? 6 : 8 };
  });

/** Servidor paginado en el que CADA página contiene una IP repetida. */
const bansServer =
  (records: number) =>
  (_endpoint: string, params?: QueryParams): Promise<Response<BanRow[]>> => {
    const page = Number(params?.get("page") ?? 1);
    const limit = Number(params?.get("limit") ?? 10);
    const offset = (page - 1) * limit;
    const data: BanRow[] = Array.from({ length: limit }, (_, i) => {
      const duplicate = i === 4;
      const ip = `10.0.0.${offset + (duplicate ? i - 1 : i + 1)}`;
      return { id: ip, ip_address: ip, rule_id: duplicate ? 6 : 8 };
    });
    return Promise.resolve({
      status: 200,
      data,
      pagination: { page, limit, pages: Math.ceil(records / limit), records },
    } as Response<BanRow[]>);
  };

/** Claves de fila que antd publica en el DOM. */
const dataRowKeys = (): (string | null)[] =>
  Array.from(
    document.querySelectorAll(".ant-table-tbody tr.ant-table-row"),
  ).map((tr) => tr.getAttribute("data-row-key"));

/** Texto del cuerpo de la tabla, para esperar a que se pinte una página. */
const tbodyText = (): string =>
  document.querySelector(".ant-table-tbody")?.textContent ?? "";

/** Texto literal con el que React denuncia dos hijos con la misma `key`. */
const DUPLICATE_KEY_WARNING = /Encountered two children with the same key/;

describe("CustomTable — identidad de fila", () => {
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

  // ── 4.1 ────────────────────────────────────────────────────────────────────
  it("4.1 una página con dos baneos de la misma IP no duplica filas ni provoca el aviso de claves repetidas", async () => {
    loadDataMock.mockResolvedValue({
      status: 200,
      data: duplicateIpPage(),
      pagination: { page: 1, limit: 10, pages: 1, records: 10 },
    } as Response<BanRow[]>);

    render(
      <CustomTable<BanRow>
        title="Active Bans"
        endpoint="bans"
        fields={BAN_FIELDS}
        t={(key) => key}
        rowKey={banRowKey}
      />,
    );

    await waitFor(() => expect(rowCount()).toBe(10));

    // Exactamente las 10 filas de la página: ni duplicadas ni omitidas.
    expect(rowCount()).toBe(10);
    // Y React no ha registrado el aviso de claves duplicadas.
    expect(duplicateKeyWarnings()).toEqual([]);
  });

  // ── 4.2 ────────────────────────────────────────────────────────────────────
  it("4.2 no acumula filas al navegar: el tamaño de página se mantiene en ambas páginas", async () => {
    loadDataMock.mockImplementation(bansServer(34));

    render(
      <CustomTable<BanRow>
        title="Active Bans"
        endpoint="bans"
        fields={BAN_FIELDS}
        t={(key) => key}
        rowKey={banRowKey}
      />,
    );

    await waitFor(() => expect(rowCount()).toBe(10));
    const firstPageRows = rowCount();

    await userEvent.click(nextButton());
    // La página 2 ya está pintada cuando aparece una celda que sólo vive en ella.
    await waitFor(() => expect(tbodyText()).toContain("10.0.0.20"));
    const secondPageRows = rowCount();

    const prev = document.querySelector(".ant-pagination-prev");
    await userEvent.click(prev as Element);
    await waitFor(() => expect(tbodyText()).toContain("10.0.0.10"));
    const backToFirstPageRows = rowCount();

    // Tamaño de página en ambas páginas…
    expect(firstPageRows).toBe(10);
    expect(secondPageRows).toBe(10);
    // …y el número no crece con la navegación.
    expect(backToFirstPageRows).toBe(10);
    expect(duplicateKeyWarnings()).toEqual([]);
  });

  // ── 4.3 ────────────────────────────────────────────────────────────────────
  it("4.3 distingue dos baneos de la misma IP cuando uno no tiene rule_id", async () => {
    loadDataMock.mockResolvedValue({
      status: 200,
      data: [
        { id: "10.0.0.9", ip_address: "10.0.0.9", rule_id: 6 },
        { id: "10.0.0.9", ip_address: "10.0.0.9" },
      ],
      pagination: { page: 1, limit: 10, pages: 1, records: 2 },
    } as Response<BanRow[]>);

    render(
      <CustomTable<BanRow>
        title="Active Bans"
        endpoint="bans"
        fields={BAN_FIELDS}
        t={(key) => key}
        rowKey={banRowKey}
      />,
    );

    await waitFor(() => expect(rowCount()).toBe(2));

    const keys = dataRowKeys();
    expect(keys).toHaveLength(2);
    // Aunque el `id` (la IP) sea el mismo, las dos filas tienen clave distinta.
    expect(keys[0]).not.toBe(keys[1]);
    expect(duplicateKeyWarnings()).toEqual([]);
  });

  // ── 4.4 ────────────────────────────────────────────────────────────────────
  it("4.4 una tabla que no declara clave de fila sigue usando record.id", async () => {
    // Guarda de regresión: las tablas cuyas filas ya tienen un `id` único no
    // cambian de comportamiento.
    loadDataMock.mockImplementation(server(34));

    renderTable();
    await waitFor(() => expect(rowCount()).toBe(10));

    expect(dataRowKeys()).toEqual(dataset(10).map((row) => String(row.id)));
    expect(duplicateKeyWarnings()).toEqual([]);
  });
});
