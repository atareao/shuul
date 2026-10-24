import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from "vitest";
import { render, screen, within, waitFor, cleanup } from "@testing-library/react";
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
// Refleja el contrato del delta: `{ waf, jail }`, sin `profiles`. Los recuentos
// son los reales —24 WAF + 18 JAIL = 42 plantillas, 16 con `must_have: true`—,
// pero los pesos del fixture son deliberadamente ficticios: las Jail reales
// pesan 0 y aquí se les asigna 200 + i justamente para que el test detecte si la
// interfaz dejase de normalizar el peso de una regla de Jail.

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
 *
 * Ensanchar el rango renderiza el catálogo entero **dos veces** —la página de
 * 10 filas primero y las 42 después— dentro de jsdom, lo que en un runner de CI
 * cuesta varios segundos (y más en el primer test, que además paga la carga
 * inicial de los módulos de antd). Por eso los tests que lo usan pasan un
 * límite explícito de 15000 ms como tercer argumento de `it`: el resto de la
 * suite conserva el límite por defecto, que es el que delata una lentitud real.
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
  }, 15000);

  // ─── Escenario: Filtering by pipeline ────────────────────────────────────
  it("filters by pipeline to the 18 JAIL templates and restores the full catalog when cleared", async () => {
    const user = userEvent.setup();
    renderPage();
    await showWholeCatalog();

    await selectOption(user, /pipeline/i, "Jail");
    await expectRows(JAIL_COUNT_TOTAL);

    await selectOption(user, /pipeline/i, "All");
    await expectRows(42);
  }, 15000);

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
  }, 15000);

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
  }, 15000);

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
  }, 15000);

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
  }, 15000);

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
  }, 15000);

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
  }, 15000);

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
  }, 15000);

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
  }, 15000);
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

// ─── El peso no se ofrece donde no decide ────────────────────────────────────
// Escenario del delta: «El peso no se ofrece donde no aplica» — GIVEN una
// plantilla cuyo pipeline es `jail` WHEN se abre su diálogo THEN el campo Weight
// no se ofrece AND el valor que la interfaz envía para esa regla es 0. El WAF se
// usa como contraste: allí el peso sí decide (`first match wins` por `weight
// ASC`), así que el campo sigue ofreciéndose con el peso de la plantilla.

/** Cuerpo de la regla que la interfaz envía a `POST /api/v1/rules`. */
interface CreatedRuleBody {
  name: string;
  pipeline: string;
  weight: number;
}

/** Forma mínima del `fetch` sustituido: solo importan las opciones de cada
 *  llamada (el segundo argumento), que es donde viajan método y cuerpo. */
interface FetchStub {
  mock: { calls: RequestInit[][] };
}

/** Cuerpos JSON de las peticiones POST registradas por el `fetch` sustituido. */
function createdRuleBodies(fetchStub: FetchStub): CreatedRuleBody[] {
  return fetchStub.mock.calls
    .filter(([, init]) => init.method === "POST")
    .map(([, init]) => JSON.parse(init.body as string) as CreatedRuleBody);
}

describe("apply modal: el peso no se ofrece donde no decide", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("does not offer Weight when applying a JAIL template — and sends 0 — while a WAF template keeps its weight", async () => {
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

    // ── Plantilla de JAIL: el pipeline evalúa todas las reglas que casan, sin
    //    `break`, así que un peso editable sería una decisión sin efecto ──
    const jailTemplate = fixture.jail[0];
    await user.click(
      screen.getByRole("button", {
        name: `Apply template ${jailTemplate.name}`,
      }),
    );
    const jailDialog = await screen.findByRole("dialog");
    // Ni la etiqueta del campo…
    expect(within(jailDialog).queryByText("Weight")).toBeNull();
    // …ni ningún control numérico que lo edite.
    expect(within(jailDialog).queryByRole("spinbutton")).toBeNull();

    await user.click(
      within(jailDialog).getByRole("button", { name: /create rule/i }),
    );
    await waitFor(() => expect(createdRuleBodies(fetchMock)).toHaveLength(1));
    const jailBody = createdRuleBodies(fetchMock)[0];
    expect(jailBody.pipeline).toBe("jail");
    expect(jailBody.weight).toBe(0);

    // ── Plantilla de WAF: el peso ordena la evaluación, así que se sigue
    //    ofreciendo y viaja con el valor de la plantilla ──
    cleanup();
    renderPage();
    await showWholeCatalog();
    await expectRows(42);

    const wafTemplate = fixture.waf[0];
    await user.click(
      screen.getByRole("button", {
        name: `Apply template ${wafTemplate.name}`,
      }),
    );
    const wafDialog = await screen.findByRole("dialog");
    expect(within(wafDialog).getByText("Weight")).toBeTruthy();
    expect(
      (within(wafDialog).getByRole("spinbutton") as HTMLInputElement).value,
    ).toBe(String(wafTemplate.weight));

    await user.click(
      within(wafDialog).getByRole("button", { name: /create rule/i }),
    );
    await waitFor(() => expect(createdRuleBodies(fetchMock)).toHaveLength(2));
    const wafBody = createdRuleBodies(fetchMock)[1];
    expect(wafBody.pipeline).toBe("waf");
    expect(wafBody.weight).toBe(wafTemplate.weight);
  }, 15000);
});

// ─── Previsualización de filtros: los valores largos no desbordan ────────────
// Delta `template-filters-overflow`. Los tags que muestran el valor de un filtro
// deben truncar su contenido con elipsis (CSS, no JavaScript) y ofrecer el valor
// íntegro en un `Tooltip` de antd, tanto en la previsualización del diálogo de
// aplicar plantilla —sección de filtros y sección de scope— como en el detalle
// expandible de una fila del listado.

describe("previsualización de filtros: los valores largos no desbordan", () => {
  // Path real de la plantilla `Scanner directorios+configs+paneles` (314 chars).
  const LONG_PATH =
    "^/(admin|administrator|backup|backups|tmp|temp|test|tests|dev|development|private|restrito|old|new)(/|$)|\\.(env|bak|sql|dump|config|yml|yaml|json|log|ini)$|/(wp-config|configuration\\.php|settings|database\\.yml)|^/(panel|cpanel|plesk|webmin|vestacp|kloxo|directadmin|phpmyadmin|pma|myadmin|mysql|phpPgAdmin|pgadmin)";

  const LONG_TEMPLATE_NAME = "Scanner directorios+configs+paneles";

  const LONG_FQDN =
    "scanner-probes-scope-with-a-very-long-name.internal.example-with-a-long-domain.test.example.com";

  /**
   * Catálogo mínimo servido sólo para estos tests: una única plantilla JAIL con
   * el path de 314 caracteres. No se toca el fixture compartido —sus recuentos
   * los afirman otros tests—: se sobrescribe la respuesta de `loadData` aquí,
   * como ya hace el test de «surfaces a load error».
   */
  function serveLongTemplate(
    overrides: Partial<RuleTemplate> = {},
  ): RuleTemplate {
    const template = makeRuleTemplate({
      name: LONG_TEMPLATE_NAME,
      pipeline: "jail",
      path: LONG_PATH,
      rate_limit_profile_id: 9,
      rate_limit_profile_name: "Scanner Aggressive",
      ...overrides,
    });
    vi.mocked(loadData).mockResolvedValue({
      status: 200,
      data: { waf: [], jail: [template] },
    });
    return template;
  }

  /** Devuelve el `span.ant-tag` cuyo texto empieza por `prefix`. */
  function findTagByPrefix(prefix: string): HTMLElement {
    const tags = screen.queryAllByText((_content, element) => {
      return (
        element instanceof HTMLElement &&
        element.tagName === "SPAN" &&
        element.classList.contains("ant-tag") &&
        (element.textContent ?? "").startsWith(prefix)
      );
    });
    if (tags.length === 0) {
      throw new Error(`No ant-tag whose text starts with "${prefix}" was found`);
    }
    return tags[0];
  }

  /**
   * El estilo inline que trunca el contenido en vez de desbordar: el tag puede
   * encogerse hasta el ancho del contenedor y recorta con elipsis. Se lee el
   * estilo inline (`element.style`), no la hoja de antd, porque es ahí donde el
   * arreglo lo declara.
   */
  function expectTruncatingStyle(tag: HTMLElement): void {
    expect(tag.style.maxWidth).toBe("100%");
    expect(tag.style.overflow).toBe("hidden");
    expect(tag.style.textOverflow).toBe("ellipsis");
    expect(tag.style.whiteSpace).toBe("nowrap");
  }

  /**
   * Pasa el ratón por `trigger` y espera a que aparezca un tooltip cuyo texto
   * contenga `expected`. antd monta el tooltip en un portal y, con `role`, sobre
   * el contenedor `.ant-tooltip-container` (`@rc-component/tooltip`), así que se
   * localiza por rol y se comprueba su texto. Se busca entre **todos** los
   * tooltips montados porque, tras un hover previo, el popup anterior puede
   * seguir en el DOM.
   */
  async function hoverForTooltip(
    user: UserEvent,
    trigger: HTMLElement,
    expected: string,
  ): Promise<void> {
    await user.hover(trigger);
    await waitFor(
      () => {
        const tips = screen.queryAllByRole("tooltip");
        expect(tips.some((t) => (t.textContent ?? "").includes(expected))).toBe(
          true,
        );
      },
      { timeout: 3000 },
    );
  }

  /** Renderiza, espera la única fila y abre el diálogo de aplicar la plantilla. */
  async function openApplyDialog(
    user: UserEvent,
    name: string,
  ): Promise<HTMLElement> {
    renderPage();
    await expectRows(1);
    await user.click(
      screen.getByRole("button", { name: `Apply template ${name}` }),
    );
    return screen.findByRole("dialog");
  }

  // ─── Tarea 1.1 ───────────────────────────────────────────────────────────
  it("trunca el tag del path de 314 caracteres en el diálogo de aplicar plantilla", async () => {
    serveLongTemplate();
    const user = userEvent.setup();
    await openApplyDialog(user, LONG_TEMPLATE_NAME);

    const pathTag = findTagByPrefix("Path:");
    expectTruncatingStyle(pathTag);
  });

  // ─── Tarea 1.2 ───────────────────────────────────────────────────────────
  it("ofrece el regex completo en un tooltip al pasar el ratón por el tag del path", async () => {
    serveLongTemplate();
    const user = userEvent.setup();
    await openApplyDialog(user, LONG_TEMPLATE_NAME);

    const pathTag = findTagByPrefix("Path:");
    await hoverForTooltip(user, pathTag, LONG_PATH);
  });

  // ─── Tarea 1.3 ───────────────────────────────────────────────────────────
  // Guarda de regresión: el truncado es visual (CSS), no recorta el DOM. Este
  // test pasa ya antes del arreglo.
  it("mantiene el regex completo en el texto del tag, no sólo en el tooltip", async () => {
    serveLongTemplate();
    const user = userEvent.setup();
    await openApplyDialog(user, LONG_TEMPLATE_NAME);

    const pathTag = findTagByPrefix("Path:");
    expect(pathTag.textContent).toContain(LONG_PATH);
  });

  // ─── Tarea 1.4 ───────────────────────────────────────────────────────────
  it("trunca los tags del detalle expandible y da a cada uno su tooltip con el valor completo", async () => {
    serveLongTemplate({
      method: "GET",
      user_agent:
        "SomeVeryLongUserAgentThatWouldOverflowTheRowIfItWereNotTruncated/1.0",
    });
    const user = userEvent.setup();
    renderPage();

    const rows = await expectRows(1);
    await user.click(
      within(rows[0]).getByRole("button", { name: /expand row/i }),
    );

    // El detalle expandible se ve con sus tags de filtro…
    const pathTag = findTagByPrefix("Path:");
    const methodTag = findTagByPrefix("Method:");
    expectTruncatingStyle(pathTag);
    expectTruncatingStyle(methodTag);

    // …y cada uno ofrece su valor completo en un tooltip.
    await hoverForTooltip(user, pathTag, LONG_PATH);
    await hoverForTooltip(user, methodTag, "Method: GET");
  });

  // ─── Tarea 1.5 ───────────────────────────────────────────────────────────
  it("trunca el tag del scope y le da su tooltip al escribir un FQDN largo", async () => {
    serveLongTemplate();
    const user = userEvent.setup();
    await openApplyDialog(user, LONG_TEMPLATE_NAME);

    // El input opcional es el que se ofrece cuando `requires_fqdn` es `false`.
    await user.type(screen.getByPlaceholderText("FQDN (optional)"), LONG_FQDN);

    await waitFor(() => {
      expect(findTagByPrefix("FQDN:").textContent).toContain(LONG_FQDN);
    });
    const scopeTag = findTagByPrefix("FQDN:");

    expectTruncatingStyle(scopeTag);
    await hoverForTooltip(user, scopeTag, LONG_FQDN);
  });

  // ─── Tarea 1.6 ───────────────────────────────────────────────────────────
  it("deja ver el valor íntegro de un filtro corto (Method: GET)", async () => {
    serveLongTemplate({ method: "GET" });
    const user = userEvent.setup();
    await openApplyDialog(user, LONG_TEMPLATE_NAME);

    // jsdom no maqueta —no calcula layout—, así que «sin elipsis visible» no es
    // comprobable aquí: se comprueba que el estilo es el mismo y que el texto no
    // se ha recortado.
    const methodTag = findTagByPrefix("Method:");
    expectTruncatingStyle(methodTag);
    expect(methodTag.textContent).toContain("GET");
  });
});

// ─── La barra de herramientas es una sola línea ──────────────────────────────
// Delta `ui-shell-and-toolbar`, requisito MODIFIED «Templates are presented as a
// single filterable listing»: el buscador deja su línea propia a ancho completo
// y comparte fila con los cuatro filtros y el interruptor `must_have`. La fila
// no envuelve: antes de partirse, los controles se encogen. El buscador es el
// elemento flexible —crece para llevarse el hueco libre y se encoge— y cada
// filtro declara su ancho preferido, pudiendo encogerse también.
//
// El tercer token del `flex` de cada `Col` es el **ancho preferido**
// (`flex-basis`), no un mínimo. No se declara ningún `min-width`, a propósito:
// el ancho efectivo lo reparte la fila, y un suelo rígido sería precisamente lo
// que haría que la línea desbordara su contenedor al estrecharse la ventana
// —justo lo que el contrato prohíbe—. El mínimo real de un elemento flexible es
// su tamaño mínimo automático de contenido, que calcula el navegador.
//
// jsdom no calcula layout, así que lo que se fija aquí es el **mecanismo**
// declarado en los estilos inline: dónde vive cada control, si la fila envuelve
// y qué `flex` declara cada `Col`. La comprobación visual es del operador.

describe("barra de herramientas: buscador y filtros comparten una sola línea", () => {
  /** Tokens del `flex` declarado en el estilo inline de una `Col`. */
  function flexTokens(col: HTMLElement): string[] {
    return col.style.flex.trim().split(/\s+/).filter((t) => t.length > 0);
  }

  /** ¿Es `token` una longitud CSS (`200px`, `12rem`, `20%`, `30ch`…)? */
  function isLength(token: string): boolean {
    return /^-?\d+(\.\d+)?(px|em|rem|%|ch|vw|vh)$/.test(token);
  }

  /** Factor de crecimiento/encogimiento de un token de `flex`. */
  function flexNumber(token: string): number {
    return Number.parseFloat(token);
  }

  /** La `Col` que envuelve un control de la barra de herramientas. */
  function colOf(control: HTMLElement): HTMLElement {
    const col = control.closest<HTMLElement>(".ant-col");
    expect(
      col,
      "cada control de la barra de herramientas debe vivir en su propia Col",
    ).not.toBeNull();
    if (!col) {
      throw new Error("No hay Col");
    }
    return col;
  }

  /**
   * Renderiza la página y devuelve el buscador con la fila que debe compartir
   * con los filtros: la `Row` más cercana que lo contiene. Que exista esa fila
   * es justamente lo que fija el contrato.
   */
  async function toolbar(): Promise<{ input: HTMLElement; row: HTMLElement }> {
    renderPage();
    const input = await screen.findByRole("textbox", {
      name: /search templates/i,
    });
    const row = input.closest<HTMLElement>(".ant-row");
    expect(
      row,
      "el buscador debe estar dentro de la misma Row que los filtros",
    ).not.toBeNull();
    if (!row) {
      throw new Error("No hay Row");
    }

    // La `Row` hallada tiene que ser *la* barra de herramientas, no una fila
    // envolvente: `closest` devolvería la externa si mañana hubiera una, y los
    // tests pasarían con el buscador en su propia línea. La firma de la barra
    // es exactamente seis `Col` directas con un control cada una.
    expect(row.querySelectorAll(":scope > .ant-col")).toHaveLength(6);
    expect(within(row).getAllByRole("textbox")).toHaveLength(1);
    expect(within(row).getAllByRole("combobox")).toHaveLength(4);
    expect(within(row).getAllByRole("switch")).toHaveLength(1);

    return { input, row };
  }

  const FILTER_NAMES = [/pipeline/i, /severity/i, /category/i, /action/i];

  // ─── Tarea 1.9 ───────────────────────────────────────────────────────────
  it("sitúa el buscador, los cuatro filtros y el interruptor must_have en la misma fila", async () => {
    const { row } = await toolbar();

    expect(
      within(row).getByRole("textbox", { name: /search templates/i }),
    ).toBeTruthy();
    for (const name of FILTER_NAMES) {
      expect(within(row).getByRole("combobox", { name })).toBeTruthy();
    }
    expect(
      within(row).getByRole("switch", { name: /must have/i }),
    ).toBeTruthy();
  });

  // ─── Tarea 1.9 ───────────────────────────────────────────────────────────
  it("declara la fila sin envolver", async () => {
    const { row } = await toolbar();

    expect(row.classList.contains("ant-row-no-wrap")).toBe(true);
    // La clase se puede desmentir con un `flex-wrap` en línea: la fila no debe
    // declararlo, o declararlo `nowrap`.
    expect(["", "nowrap"]).toContain(row.style.flexWrap);
  });

  // ─── Tarea 1.9 ───────────────────────────────────────────────────────────
  it("hace del buscador el elemento flexible: crece, se encoge y declara su ancho preferido", async () => {
    const { input } = await toolbar();

    const searchCol = colOf(input);
    const tokens = flexTokens(searchCol);
    expect(tokens).toHaveLength(3);
    expect(flexNumber(tokens[0])).toBeGreaterThan(0);
    expect(flexNumber(tokens[1])).toBeGreaterThan(0);
    expect(isLength(tokens[2])).toBe(true);
    // Un `Col` flexible arranca con `min-width: auto` —su tamaño de contenido—
    // y eso le pone un suelo duro a la fila: `min-width: 0` es lo que lo
    // levanta para que pueda encogerse por debajo de su contenido.
    expect(Number.parseFloat(searchCol.style.minWidth)).toBe(0);
  });

  // ─── Tarea 1.9 ───────────────────────────────────────────────────────────
  it("deja que los cuatro filtros se encojan, cada uno desde su propio ancho preferido", async () => {
    const { row } = await toolbar();

    for (const name of FILTER_NAMES) {
      const filterCol = colOf(within(row).getByRole("combobox", { name }));
      const tokens = flexTokens(filterCol);
      expect(tokens).toHaveLength(3);
      expect(flexNumber(tokens[0])).toBe(0);
      expect(flexNumber(tokens[1])).toBe(1);
      expect(isLength(tokens[2])).toBe(true);
      // El suelo de contenido también aquí: cada filtro declara `min-width: 0`.
      expect(Number.parseFloat(filterCol.style.minWidth)).toBe(0);
    }

    // El interruptor `must_have` no se encoge: conserva su ancho natural.
    const switchCol = colOf(
      within(row).getByRole("switch", { name: /must have/i }),
    );
    expect(flexNumber(flexTokens(switchCol)[1])).toBe(0);

    // Ningún `Col` de la barra declara un mínimo rígido (valor positivo): un
    // suelo así es justo lo que haría desbordar la línea al estrecharse.
    for (const col of row.querySelectorAll<HTMLElement>(":scope > .ant-col")) {
      const minWidth = Number.parseFloat(col.style.minWidth || "0");
      expect(
        minWidth <= 0,
        `la barra no debe declarar un min-width rígido, pero declara "${col.style.minWidth}"`,
      ).toBe(true);
    }
  });

  // ─── Tarea 1.9 ───────────────────────────────────────────────────────────
  it("ya no declara el buscador a ancho completo", async () => {
    const { input } = await toolbar();

    const wrapper = input.closest<HTMLElement>(".ant-input-affix-wrapper");
    // Sin esto, la comprobación del envoltorio pasaría aunque no existiera.
    expect(wrapper).not.toBeNull();
    expect(input.style.width).not.toBe("100%");
    expect(wrapper?.style.width).not.toBe("100%");
  });
});
