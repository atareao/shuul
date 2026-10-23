import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from "vitest";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { UserEvent } from "@testing-library/user-event";
import TemplatesPage from "./templates_page";
import type { RuleTemplate } from "@/models/template";
import { loadData } from "@/common/utils";

// La página consulta `loadData("templates", ...)`; se sustituye por un mock
// tipado para servir el catálogo sin red.
vi.mock("@/common/utils", () => ({
  loadData: vi.fn(),
}));

vi.mock("@/constants", () => ({
  BASE_URL: "http://localhost:3000",
}));

// antd necesita ResizeObserver y matchMedia en jsdom.
beforeAll(() => {
  class ResizeObserverStub {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  globalThis.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }),
  });
});

// ─── Fixture del catálogo ────────────────────────────────────────────────────
// Refleja el contrato del delta: `{ waf, jail }`, sin `profiles`. Las cifras
// son las reales: 24 WAF + 18 JAIL = 42 plantillas, 16 con `must_have: true`.

interface TemplatesFixture {
  waf: RuleTemplate[];
  jail: RuleTemplate[];
}

const WAF_COUNT = 24;
const JAIL_COUNT = 18;
const WAF_MUST_HAVE = 10;
const JAIL_MUST_HAVE = 6;
const MUST_HAVE_TOTAL = WAF_MUST_HAVE + JAIL_MUST_HAVE;
const JAIL_COUNT_TOTAL = JAIL_COUNT;

const WAF_CATEGORIES = ["wordpress", "laravel", "api", "bots"] as const;
const JAIL_CATEGORIES = ["paneles", "api", "bots"] as const;
const SEVERITIES = ["critico", "alto", "medio", "bajo"] as const;

const pad = (n: number): string => String(n).padStart(2, "0");

function makeRuleTemplate(
  overrides: Partial<RuleTemplate> & Pick<RuleTemplate, "name" | "pipeline">,
): RuleTemplate {
  return {
    description: `${overrides.name} template description`,
    category: "seguridad",
    severity: "medio",
    path: null,
    query: null,
    country_code: null,
    ip_address: null,
    protocol: null,
    city_name: null,
    country_name: null,
    user_agent: null,
    method: null,
    referer: null,
    content_type: null,
    accept_language: null,
    x_request_id: null,
    is_tor: null,
    allow: true,
    rate_limit_profile_id: null,
    rate_limit_profile_name: null,
    requires_fqdn: false,
    must_have: false,
    weight: 100,
    ...overrides,
  };
}

function buildFixture(): TemplatesFixture {
  const waf: RuleTemplate[] = Array.from({ length: WAF_COUNT }, (_, i) =>
    makeRuleTemplate({
      name: `WAF Template ${pad(i + 1)}`,
      pipeline: "waf",
      category: WAF_CATEGORIES[i % WAF_CATEGORIES.length],
      severity: SEVERITIES[i % SEVERITIES.length],
      allow: i % 2 === 0,
      weight: 100 + i,
      must_have: i < WAF_MUST_HAVE,
    }),
  );

  const jail: RuleTemplate[] = Array.from({ length: JAIL_COUNT }, (_, i) =>
    makeRuleTemplate({
      name: i === 0 ? "Scanner Aggressive" : `Jail Template ${pad(i + 1)}`,
      description:
        i === 0
          ? "Aggressive scanner detection for probes"
          : `Jail Template ${pad(i + 1)} template description`,
      pipeline: "jail",
      category: JAIL_CATEGORIES[i % JAIL_CATEGORIES.length],
      severity: SEVERITIES[i % SEVERITIES.length],
      allow: false,
      weight: 200 + i,
      must_have: i < JAIL_MUST_HAVE,
      rate_limit_profile_id: i + 1,
      rate_limit_profile_name: `Jail Profile ${i + 1}`,
    }),
  );

  return { waf, jail };
}

let fixture: TemplatesFixture;
let all: RuleTemplate[];

function renderPage(): void {
  render(<TemplatesPage />);
}

/**
 * Filas de datos del listado: las `row` sin cabeceras de columna. Así se
 * descarta la fila de cabecera de la tabla sin acoplarse a clases de antd.
 */
function getDataRows(): HTMLElement[] {
  return screen
    .queryAllByRole("row")
    .filter((row) => within(row).queryAllByRole("columnheader").length === 0);
}

/** Espera a que el listado tenga `count` filas y las devuelve. */
async function expectRows(count: number): Promise<HTMLElement[]> {
  await waitFor(() => expect(getDataRows()).toHaveLength(count));
  return getDataRows();
}

/** Nombre de plantilla de una fila, localizado por el texto del catálogo. */
function nameOfRow(row: HTMLElement): string | undefined {
  return all.map((t) => t.name).find((n) => within(row).queryAllByText(n).length > 0);
}

/**
 * Abre un `Select` de filtro por su nombre accesible y elige la opción por su
 * etiqueta visible.
 *
 * La búsqueda se acota al desplegable recién abierto. El rol `listbox` que
 * expone antd pertenece al *mirror* ARIA oculto (no contiene todas las
 * opciones ni es clicable), así que se usa como ancla para localizar el
 * contenedor `.ant-select-dropdown` del desplegable visible y buscar la
 * opción por su `title` dentro de él. Sin ese acotado, `title="All"` del
 * valor seleccionado de otro `Select` colisionaba con la opción "All".
 */
async function selectOption(
  user: UserEvent,
  comboboxName: RegExp,
  optionLabel: string,
): Promise<void> {
  const combo = await screen.findByRole("combobox", { name: comboboxName });
  await user.click(combo);
  const listbox = await screen.findByRole("listbox");
  const dropdown = listbox.closest<HTMLElement>(".ant-select-dropdown");
  if (!dropdown) {
    throw new Error("Ant Select dropdown container not found");
  }
  const option = await within(dropdown).findByTitle(optionLabel);
  await user.click(option);
}

async function typeSearch(user: UserEvent, text: string): Promise<void> {
  const box = await screen.findByRole("textbox", { name: /search/i });
  await user.clear(box);
  await user.type(box, text);
}

/**
 * Fija el tamaño de página al máximo ofrecido (100) para poder afirmar el
 * catálogo entero de una sola vez.
 *
 * El listado pagina a 10 por defecto, pero estos tests afirman el conjunto
 * completo (las 42 plantillas, los 18 JAIL, las 16 `must_have`…) tal y como
 * hacían antes de que existiera la paginación. Fijar el rango —que es uno de
 * los ofrecidos por el propio selector— permite seguir afirmando exactamente lo
 * mismo sin tocar ninguna aserción.
 *
 * Debe llamarse cuando el listado ya tenga filas: mientras `dataSource` está
 * vacío antd no renderiza la paginación y el combobox de tamaño no existe.
 */
async function showWholeCatalog(): Promise<void> {
  await waitFor(() => expect(getDataRows().length).toBeGreaterThan(0));

  // El selector de tamaño lo genera la `Pagination` de antd; su nombre
  // accesible `"Page Size"` lo inyecta el locale `en_US` de
  // `@rc-component/pagination`, y cada opción es un `role="option"` con
  // `title="<n> / page"`.
  const user = userEvent.setup();
  const combo = await screen.findByRole("combobox", { name: "Page Size" });
  await user.click(combo);
  const listbox = await screen.findByRole("listbox");
  const dropdown = listbox.closest<HTMLElement>(".ant-select-dropdown");
  if (!dropdown) {
    throw new Error("Ant Select dropdown container not found");
  }
  await user.click(await within(dropdown).findByTitle("100 / page"));

  // Al cerrarse, el desplegable entra en su animación de salida y queda colgado
  // en el DOM (`display: flex` con las clases `ant-slide-up-leave`), porque
  // jsdom **no dispara** el evento de fin de animación y rc-motion no llega a
  // desmontarlo. Un desplegable huérfano rompería el siguiente `selectOption`:
  // `findByRole` encontraría dos `listbox` (el del filtro recién abierto y
  // éste). Se emiten a mano los eventos que sí emitiría un navegador de verdad
  // y se espera a que el nodo salga del árbol, de modo que el DOM quede limpio
  // para el test siguiente.
  //
  // El nombre bajo el que rc-motion espera el evento depende del entorno: en
  // jsdom, que expone `TransitionEvent` pero **no** `AnimationEvent`, la
  // resolución con prefijo de proveedor se queda en `webkitAnimationEnd` (un
  // navegador registra `animationend`); `transitionend` lo registra en ambos
  // casos. Se emiten los tres candidatos, y `waitFor` los reintenta porque los
  // listeners se enganchan un par de frames después, al alcanzar el paso activo.
  await waitFor(() => {
    for (const type of ["animationend", "webkitAnimationEnd", "transitionend"]) {
      dropdown.dispatchEvent(new Event(type, { bubbles: true }));
    }
    expect(screen.queryAllByRole("listbox")).toHaveLength(0);
  });

  await waitFor(() => expect(getDataRows().length).toBeGreaterThan(10));
}

beforeEach(() => {
  fixture = buildFixture();
  all = [...fixture.waf, ...fixture.jail];
  vi.mocked(loadData).mockReset();
  vi.mocked(loadData).mockResolvedValue({ status: 200, data: fixture });
});

describe("TemplatesPage — listado único y filtrable", () => {
  // ─── Escenario: All rule templates appear in one list ────────────────────
  it("renders the whole catalog as a single listing of 42 rows, with no tabs and no rate-limit-profiles section", async () => {
    renderPage();
    await showWholeCatalog();

    const rows = await expectRows(42);
    expect(rows).toHaveLength(42);

    // Un solo listado: WAF y JAIL conviven, sin pestañas que los separen.
    const names = rows.map(nameOfRow);
    expect(names).toContain(fixture.waf[0].name);
    expect(names).toContain(fixture.jail[0].name);
    expect(screen.queryAllByRole("tab")).toHaveLength(0);

    // Nada de perfiles de rate limit en la página.
    expect(screen.queryByText("Rate Limit Profiles")).toBeNull();
    expect(screen.queryByRole("button", { name: /apply profile/i })).toBeNull();
  });

  // ─── Escenario: Filtering by pipeline ────────────────────────────────────
  it("filters by pipeline to the 18 JAIL templates and restores the full catalog when cleared", async () => {
    const user = userEvent.setup();
    renderPage();
    await showWholeCatalog();

    await selectOption(user, /pipeline/i, "Jail");
    await expectRows(JAIL_COUNT_TOTAL);

    await selectOption(user, /pipeline/i, "All");
    await expectRows(42);
  });

  // ─── Escenario: Filtering by severity ────────────────────────────────────
  it("filters by severity", async () => {
    const user = userEvent.setup();
    renderPage();
    await showWholeCatalog();

    const expected = all.filter((t) => t.severity === "critico").length;
    await selectOption(user, /severity/i, "critico");
    await expectRows(expected);

    await selectOption(user, /severity/i, "All");
    await expectRows(42);
  });

  // ─── Filtro por categoría (del requisito) ────────────────────────────────
  it("filters by category", async () => {
    const user = userEvent.setup();
    renderPage();
    await showWholeCatalog();

    const expected = all.filter((t) => t.category === "api").length;
    await selectOption(user, /category/i, "api");
    await expectRows(expected);

    await selectOption(user, /category/i, "All");
    await expectRows(42);
  });

  // ─── Filtro por acción (del requisito) ───────────────────────────────────
  it("filters by action", async () => {
    const user = userEvent.setup();
    renderPage();
    await showWholeCatalog();

    const expectedAllow = all.filter((t) => t.allow).length;
    await selectOption(user, /action/i, "Allow");
    await expectRows(expectedAllow);

    await selectOption(user, /action/i, "All");
    await expectRows(42);
  });

  // ─── Escenario: Must-have is a filter ────────────────────────────────────
  it("treats must_have as a filter with 16 matches and keeps the full catalog reachable", async () => {
    const user = userEvent.setup();
    renderPage();
    await showWholeCatalog();

    const toggle = await screen.findByRole("switch", { name: /must have/i });
    await user.click(toggle);
    await expectRows(MUST_HAVE_TOTAL);

    await user.click(toggle);
    await expectRows(42);
  });

  // ─── Escenario: Filters combine with free-text search ────────────────────
  it("combines the pipeline filter with free-text search", async () => {
    const user = userEvent.setup();
    renderPage();
    await showWholeCatalog();

    await selectOption(user, /pipeline/i, "Jail");
    await expectRows(JAIL_COUNT_TOTAL);

    await typeSearch(user, "scanner");
    const rows = await expectRows(1);
    expect(nameOfRow(rows[0])).toBe("Scanner Aggressive");
  });

  // ─── Escenario: Default order (must_have first, then by weight) ──────────
  it("orders must_have templates first and the rest by weight", async () => {
    renderPage();
    await showWholeCatalog();

    const rows = await expectRows(42);
    const names = rows.map(nameOfRow);

    const mustHaveNames = all.filter((t) => t.must_have).map((t) => t.name);
    const restNames = all
      .filter((t) => !t.must_have)
      .sort((a, b) => a.weight - b.weight)
      .map((t) => t.name);

    expect(mustHaveNames).toHaveLength(MUST_HAVE_TOTAL);
    expect(names.slice(0, mustHaveNames.length).sort()).toEqual(
      [...mustHaveNames].sort(),
    );
    expect(names.slice(mustHaveNames.length)).toEqual(restNames);
  });

  // ─── Flujo Apply (task 3.5): sigue creando la regla ─────────────────────
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("applies a rule template by POSTing it to /api/v1/rules", async () => {
    const user = userEvent.setup();
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({}),
    });
    vi.stubGlobal("fetch", fetchMock);

    renderPage();
    await showWholeCatalog();
    await expectRows(42);

    const applyButtons = await screen.findAllByRole("button", {
      name: /^apply template/i,
    });
    await user.click(applyButtons[0]);

    // El modal de aplicación se abre y confirma con "Create Rule".
    const createButton = await screen.findByRole("button", {
      name: /create rule/i,
    });
    await user.click(createButton);

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toContain("/api/v1/rules");
    expect(init.method).toBe("POST");
    const body = JSON.parse(init.body as string) as {
      name: string;
      pipeline: string;
    };
    expect(body.pipeline).toBe("waf");
    expect(body.name).toBe(all.filter((t) => t.must_have)[0].name);
  });

  // ─── Escenario: fallo de carga visible ───────────────────────────────────
  it("surfaces a load error instead of an empty catalog when the request fails", async () => {
    vi.mocked(loadData).mockResolvedValue({
      status: 500,
      message: "Catalog unavailable",
    });
    renderPage();

    const errors = await screen.findAllByText("Catalog unavailable");
    expect(errors.length).toBeGreaterThan(0);
    expect(getDataRows()).toHaveLength(0);
    // El error no se confunde con el catálogo vacío.
    expect(screen.queryByText("No templates found")).toBeNull();
  });

  it("still shows the empty catalog placeholder when the catalog is legitimately empty", async () => {
    vi.mocked(loadData).mockResolvedValue({
      status: 200,
      data: { waf: [], jail: [] },
    });
    renderPage();

    expect(await screen.findByText("No templates found")).toBeTruthy();
    expect(getDataRows()).toHaveLength(0);
  });

  // ─── Escenario: detalle de filtros en la fila expandible ─────────────────
  // Guarda de regresión: el código ya lo hace; este test lo protege.
  it("shows its own filters in the expanded row, not another template's", async () => {
    const alpha = makeRuleTemplate({
      name: "Alpha Template",
      pipeline: "waf",
      path: "/alpha-only",
      user_agent: "AlphaBot",
    });
    const beta = makeRuleTemplate({
      name: "Beta Template",
      pipeline: "waf",
      path: "/beta-only",
      user_agent: "BetaBot",
    });
    vi.mocked(loadData).mockResolvedValue({
      status: 200,
      data: { waf: [alpha, beta], jail: [] },
    });
    const user = userEvent.setup();
    renderPage();

    const rows = await expectRows(2);
    // Antes de expandir no se ve ningún detalle de filtros.
    expect(screen.queryByText("Path: /alpha-only")).toBeNull();
    expect(screen.queryByText("Path: /beta-only")).toBeNull();

    const alphaRow = rows.find(
      (r) => within(r).queryAllByText("Alpha Template").length > 0,
    );
    expect(alphaRow).toBeTruthy();
    await user.click(
      within(alphaRow as HTMLElement).getByRole("button", {
        name: /expand row/i,
      }),
    );

    // Se ven los filtros de Alpha…
    expect(await screen.findByText("Path: /alpha-only")).toBeTruthy();
    expect(screen.getByText("User agent: AlphaBot")).toBeTruthy();
    // …y no los de Beta.
    expect(screen.queryByText("Path: /beta-only")).toBeNull();
  });

  // ─── Guarda de regresión: columna Profile ────────────────────────────────
  it("shows the rate limit profile on JAIL rows and a placeholder on WAF rows", async () => {
    renderPage();
    await showWholeCatalog();

    const rows = await expectRows(42);
    const jailRow = rows.find(
      (r) => within(r).queryAllByText(fixture.jail[0].name).length > 0,
    );
    const wafRow = rows.find(
      (r) => within(r).queryAllByText(fixture.waf[0].name).length > 0,
    );
    expect(jailRow).toBeTruthy();
    expect(wafRow).toBeTruthy();

    expect(
      within(jailRow as HTMLElement).getByText(
        fixture.jail[0].rate_limit_profile_name as string,
      ),
    ).toBeTruthy();
    expect(
      within(wafRow as HTMLElement).queryByText(/Jail Profile/),
    ).toBeNull();
    expect(
      within(wafRow as HTMLElement).getAllByText("—").length,
    ).toBeGreaterThan(0);
  });
});

describe("pagination", () => {
  /**
   * Abre el selector de tamaño de página de antd y devuelve sus opciones.
   *
   * El selector lo genera la `Pagination` de antd, no la página, así que no
   * tiene `aria-label` propio en el código de la app: su nombre accesible
   * `"Page Size"` lo inyecta el locale `en_US` de `@rc-component/pagination`
   * (`page_size: "Page Size"`). Por eso se localiza por rol + nombre y no por
   * clases CSS internas. Cada opción es un `role="option"` cuyo `title` es
   * `"<n> / page"` (el locale compone `${n} ${items_per_page}` con
   * `items_per_page: "/ page"`).
   */
  async function sizeChangerOptions(user: UserEvent): Promise<HTMLElement[]> {
    const combo = await screen.findByRole("combobox", { name: "Page Size" });
    await user.click(combo);
    const listbox = await screen.findByRole("listbox");
    const dropdown = listbox.closest<HTMLElement>(".ant-select-dropdown");
    if (!dropdown) {
      throw new Error("Ant Select dropdown container not found");
    }
    return within(dropdown).getAllByRole("option");
  }

  // ─── Escenario: «The templates listing is paginated at 10» ───────────────
  it("arranca en 10 filas y la última página (la 5ª) muestra las 2 restantes", async () => {
    const user = userEvent.setup();
    renderPage();

    // El catálogo tiene 42 plantillas. Sin paginación se renderizan las 42 de
    // golpe, y `getDataRows()` cuenta exactamente las filas renderizadas: el 10
    // fija el tamaño de página por defecto del requisito. Se mide con
    // `getDataRows()` directamente —y no con `expectRows()`— porque ese helper
    // se ensanchará en GREEN.
    await waitFor(() => expect(getDataRows()).toHaveLength(10));
    expect(getDataRows()).toHaveLength(10);

    // 42 plantillas a 10 por página => 5 páginas; la última retiene 42 - 40 = 2.
    // Los items de paginación de antd llevan `title="<n>"` (marcado estable: es
    // el atributo que antd usa para sus propios tests; las clases
    // `ant-pagination-item-*` son detalle de presentación), así que la 5ª
    // página se localiza con `getByTitle("5")`.
    await user.click(screen.getByTitle("5"));

    await waitFor(() => expect(getDataRows()).toHaveLength(2));
    expect(getDataRows()).toHaveLength(2);
  });

  // ─── Escenario: «The templates listing is paginated at 10» (rangos) ──────
  it("ofrece exactamente 10, 50 y 100 en el selector de tamaño de página", async () => {
    const user = userEvent.setup();
    renderPage();

    // El desplegable de tamaño solo existe si el listado está paginado: primero
    // se confirma que hay filas, luego que la paginación está disponible.
    await waitFor(() => expect(getDataRows().length).toBeGreaterThan(0));

    const options = await sizeChangerOptions(user);
    expect(options.map((o) => o.getAttribute("title"))).toEqual([
      "10 / page",
      "50 / page",
      "100 / page",
    ]);

    // Fuerza la aserción: el rango 20 (el que hoy ofrece el Log Viewer) no debe
    // aparecer. Sin esta comprobación un `toEqual` con 10/50/100/20 pasaría si
    // el orden cambiara.
    expect(screen.queryByTitle("20 / page")).toBeNull();
  });

  // ─── Escenario: «Filtering the templates listing returns to the first page» ─
  it("al filtrar desde la última página vuelve a la primera, sin quedar vacío", async () => {
    const user = userEvent.setup();
    renderPage();

    await waitFor(() => expect(getDataRows()).toHaveLength(10));
    await user.click(screen.getByTitle("5"));
    await waitFor(() => expect(getDataRows()).toHaveLength(2));

    // El filtro `Jail` deja 18 plantillas: más de una página (18 > 10) pero
    // menos que las 5 páginas del conjunto completo. Si el número de página no
    // se reiniciase, la tabla seguiría en la 5ª página —ya fuera de rango— y
    // aparecería vacía. Que se vean 10 filas y que la primera sea el primer
    // JAIL del conjunto filtrado (`fixture.jail[0]`, el `must_have` de menor
    // peso) demuestra que se volvió a la página 1 con los datos filtrados.
    await selectOption(user, /pipeline/i, "Jail");

    await waitFor(() => expect(getDataRows()).toHaveLength(10));
    const rows = getDataRows();
    expect(rows).toHaveLength(10);
    expect(nameOfRow(rows[0])).toBe(fixture.jail[0].name);
  });
});
