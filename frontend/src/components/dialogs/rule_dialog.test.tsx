import { describe, it, expect, vi, beforeAll, afterEach } from "vitest";
import { render, screen, within, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { UserEvent } from "@testing-library/user-event";
import RuleDialog from "./rule_dialog";
import type Item from "@/models/rule";
import { DialogModes } from "@/common/types";

// El diálogo compone sus URLs con `BASE_URL`; se sustituye por un valor fijo
// para que el test no dependa del entorno.
vi.mock("@/constants", () => ({
  BASE_URL: "http://localhost:3000",
}));

// antd necesita ResizeObserver y matchMedia en jsdom (mismo habilitador que
// `templates_page.test.tsx`).
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
});

// ─── Contrato que fija este test ─────────────────────────────────────────────
// Escenario del delta: «El peso no se ofrece donde no aplica» — GIVEN una regla
// o una plantilla cuyo pipeline es `jail` WHEN se abre su diálogo THEN el campo
// Weight no se ofrece AND el valor que la interfaz envía para esa regla es 0.
// El diálogo de regla abre en `waf`, donde el peso sí decide (`first match wins`
// por `weight ASC`): allí el campo sigue ofreciéndose y el valor viaja intacto.

/** Cuerpo de la regla que el diálogo envía a `POST /api/v1/rules`. */
interface RuleBody {
  pipeline: string;
  weight: number;
  transformations: string;
}

/** Forma mínima del `fetch` sustituido: solo importan las opciones de cada
 *  llamada (el segundo argumento), que es donde viajan método y cuerpo. */
interface FetchStub {
  mock: { calls: RequestInit[][] };
}

/** Cuerpos JSON de las peticiones POST registradas por el `fetch` sustituido. */
function postedRuleBodies(fetchStub: FetchStub): RuleBody[] {
  return fetchStub.mock.calls
    .filter(([, init]) => init.method === "POST")
    .map(([, init]) => JSON.parse(init.body as string) as RuleBody);
}

/** Cuerpos JSON de las peticiones PATCH registradas por el `fetch` sustituido. */
function patchedRuleBodies(fetchStub: FetchStub): RuleBody[] {
  return fetchStub.mock.calls
    .filter(([, init]) => init.method === "PATCH")
    .map(([, init]) => JSON.parse(init.body as string) as RuleBody);
}

/** `handleCloseDialog` no-op: el diálogo se mantiene montado entre aserciones. */
const closeDialog = (): void => {};

/** Abre el diálogo de regla en modo creación, con el pipeline por defecto. */
function renderCreateDialog(): void {
  render(
    <RuleDialog
      dialogMode={DialogModes.CREATE}
      selectedItem={undefined}
      handleCloseDialog={closeDialog}
      endpoint="rules"
      t={(key: string) => key}
    />,
  );
}

/** Abre el diálogo de regla en modo edición sobre una regla ya existente. */
function renderUpdateDialog(item: Item): void {
  render(
    <RuleDialog
      dialogMode={DialogModes.UPDATE}
      selectedItem={item}
      handleCloseDialog={closeDialog}
      endpoint="rules"
      t={(key: string) => key}
    />,
  );
}

/** Regla de Jail ya almacenada: su peso no decide y no debe propagarse. */
const JAIL_ITEM: Item = {
  id: 7,
  name: "Jail rule",
  description: "",
  mode: "log_only",
  pipeline: "jail",
  weight: 250,
};

/** Regla de WAF ya almacenada: aquí el peso ordena y viaja intacto. */
const WAF_ITEM: Item = {
  id: 8,
  name: "WAF rule",
  description: "",
  mode: "enforce",
  pipeline: "waf",
  weight: 250,
};

/**
 * `combobox` del campo cuya etiqueta visible es `label`.
 *
 * `renderSelectRow` pinta la etiqueta como un `Typography.Text` hermano del
 * `Select`, no como un `<label>`, así que antd no le pone nombre accesible al
 * `combobox`. Se usa el texto visible como ancla —que es lo que ve el usuario—
 * y se resuelve el `combobox` de su misma fila por rol, sin índices ni clases
 * internas de antd.
 */
function fieldCombobox(label: string): HTMLElement {
  for (const node of screen.queryAllByText(label)) {
    const combo = node.parentElement?.querySelector<HTMLElement>(
      '[role="combobox"]',
    );
    if (combo) {
      return combo;
    }
  }
  throw new Error(`No combobox found for field "${label}"`);
}

/** Abre el desplegable de un campo y elige la opción por su etiqueta visible. */
async function selectFieldOption(
  user: UserEvent,
  label: string,
  optionLabel: string,
): Promise<void> {
  await user.click(fieldCombobox(label));
  // El `listbox` que expone antd pertenece al *mirror* ARIA oculto: se usa como
  // ancla para llegar al desplegable visible y buscar la opción dentro de él
  // (mismo criterio que el helper de `templates_page.test.tsx`).
  const listbox = await screen.findByRole("listbox");
  const dropdown = listbox.closest<HTMLElement>(".ant-select-dropdown");
  if (!dropdown) {
    throw new Error("Ant Select dropdown container not found");
  }
  await user.click(await within(dropdown).findByTitle(optionLabel));
}

/**
 * Confirma el diálogo y espera a que la petición quede registrada.
 *
 * Cada render de este test confirma **una sola vez**: mientras
 * `confirmLoading` está activo antd antepone el icono de carga al nombre
 * accesible del botón («loading Ok»), así que se evita volver a pulsarlo.
 */
async function submitDialog(
  user: UserEvent,
  fetchStub: FetchStub,
  expectedBodies: number,
): Promise<void> {
  await user.click(screen.getByRole("button", { name: "Ok" }));
  await waitFor(() =>
    expect(postedRuleBodies(fetchStub)).toHaveLength(expectedBodies),
  );
}

describe("RuleDialog: el peso no se ofrece donde no decide", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("does not offer Weight when the pipeline is Jail — and sends 0 — while it does offer it for WAF", async () => {
    const user = userEvent.setup();
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({}),
    });
    vi.stubGlobal("fetch", fetchMock);

    // ── WAF (el pipeline con el que abre el diálogo): el peso decide el orden
    //    de evaluación, así que el campo se ofrece con su valor ──
    renderCreateDialog();
    expect(await screen.findByText("Weight")).toBeTruthy();
    expect((screen.getByRole("spinbutton") as HTMLInputElement).value).toBe(
      "100",
    );

    await submitDialog(user, fetchMock, 1);
    const wafBody = postedRuleBodies(fetchMock)[0];
    expect(wafBody.pipeline).toBe("waf");
    expect(wafBody.weight).toBe(100);

    // ── Jail: el pipeline evalúa todas las reglas que casan, sin `break`, así
    //    que el peso no decide nada y deja de ofrecerse ──
    cleanup();
    renderCreateDialog();
    await selectFieldOption(user, "Pipeline", "Jail");
    expect(screen.queryByText("Weight")).toBeNull();
    expect(screen.queryByRole("spinbutton")).toBeNull();

    await submitDialog(user, fetchMock, 2);
    const jailBody = postedRuleBodies(fetchMock)[1];
    expect(jailBody.pipeline).toBe("jail");
    expect(jailBody.weight).toBe(0);
  });

  it("sends 0 on PATCH for a Jail rule while a WAF rule keeps its stored weight", async () => {
    const user = userEvent.setup();
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({}),
    });
    vi.stubGlobal("fetch", fetchMock);

    // ── Edición de una regla de Jail: el peso no decide en el pipeline, así
    //    que ni se ofrece ni se propaga; el PATCH lo fija a 0 ──
    renderUpdateDialog(JAIL_ITEM);
    expect(await screen.findByText("Pipeline")).toBeTruthy();
    expect(screen.queryByText("Weight")).toBeNull();
    expect(screen.queryByRole("spinbutton")).toBeNull();

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));
    const jailBody = patchedRuleBodies(fetchMock)[0];
    expect(jailBody.pipeline).toBe("jail");
    expect(jailBody.weight).toBe(0);

    // ── Edición de una regla de WAF: el peso ordena la evaluación, así que se
    //    sigue ofreciendo con el valor almacenado y viaja intacto ──
    cleanup();
    renderUpdateDialog(WAF_ITEM);
    expect(await screen.findByText("Weight")).toBeTruthy();
    expect((screen.getByRole("spinbutton") as HTMLInputElement).value).toBe(
      "250",
    );

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(2));
    const wafBody = patchedRuleBodies(fetchMock)[1];
    expect(wafBody.pipeline).toBe("waf");
    expect(wafBody.weight).toBe(250);
  });
});

// ─── La pipeline de transformaciones se edita en la pestaña «Request» ─────────
// Escenario del delta: una regla puede declarar una pipeline de transformaciones
// aplicada al valor de la petición antes de compararlo. La interfaz la construye
// **en el orden en que se pulsan los nombres canónicos** —el orden importa:
// `urlDecodeUni,lowercase` no es `lowercase,urlDecodeUni`—, la deja editar a
// mano y la envía en `transformations` dentro del cuerpo de `POST`/`PATCH`.
//
// El `t` que recibe el diálogo es `(key) => key`, así que la etiqueta visible y
// el `aria-label` del campo valen literalmente `Transformations` y el nombre
// accesible de cada chip es el token canónico tal cual.

/** Los 18 nombres canónicos, en el orden en que se ofrecen. */
const CANONICAL_TRANSFORMATIONS = [
  "urlDecode",
  "urlDecodeUni",
  "lowercase",
  "uppercase",
  "removeNulls",
  "replaceNulls",
  "compressWhitespace",
  "removeWhitespace",
  "normalizePath",
  "normalizePathWin",
  "htmlEntityDecode",
  "jsDecode",
  "cssDecode",
  "escapeSeqDecode",
  "base64Decode",
  "cmdLine",
  "trim",
  "length",
] as const;

describe("RuleDialog: la pipeline de transformaciones", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  /** Sustituye `fetch` por un doble que responde 200 con un cuerpo vacío. */
  function stubFetch(): FetchStub {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({}),
    });
    vi.stubGlobal("fetch", fetchMock);
    return fetchMock;
  }

  /**
   * Pulsa la pestaña «Request».
   *
   * antd sólo monta el panel de la pestaña activa, así que el campo y los chips
   * no existen en el DOM hasta pulsarla. El icono de la etiqueta no aporta nada
   * al nombre accesible, así que el `tab` se localiza por su texto.
   */
  async function openRequestTab(user: UserEvent): Promise<void> {
    await user.click(await screen.findByRole("tab", { name: /request/i }));
  }

  /** El campo de texto de la especificación, por su `aria-label`. */
  function transformationsInput(): HTMLInputElement {
    return screen.getByRole("textbox", {
      name: "Transformations",
    }) as HTMLInputElement;
  }

  it("offers the eighteen canonical names as chips and builds the specification in the order they are pressed", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderCreateDialog();
    await openRequestTab(user);

    // Los 18 nombres canónicos están presentes como elementos clicables.
    for (const token of CANONICAL_TRANSFORMATIONS) {
      expect(screen.getByRole("button", { name: token })).toBeTruthy();
    }

    await user.click(screen.getByRole("button", { name: "urlDecodeUni" }));
    await user.click(screen.getByRole("button", { name: "lowercase" }));

    expect(transformationsInput().value).toBe("urlDecodeUni,lowercase");

    await submitDialog(user, fetchMock, 1);
    expect(postedRuleBodies(fetchMock)[0].transformations).toBe(
      "urlDecodeUni,lowercase",
    );
  });

  it("respects the pressing order", async () => {
    const user = userEvent.setup();
    stubFetch();

    renderCreateDialog();
    await openRequestTab(user);

    await user.click(screen.getByRole("button", { name: "lowercase" }));
    await user.click(screen.getByRole("button", { name: "urlDecodeUni" }));

    // El orden se conserva: no es un `Select` múltiple, donde el orden de
    // selección se perdería y la pipeline se aplicaría al revés.
    expect(transformationsInput().value).toBe("lowercase,urlDecodeUni");
  });

  it("lets the operator type the specification by hand, both in create and in edit mode", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    // ── Creación: la especificación se escribe a mano en el campo ──
    renderCreateDialog();
    await openRequestTab(user);
    const box = transformationsInput();
    expect(box.value).toBe("");

    await user.type(box, "trim");
    expect(box.value).toBe("trim");

    await submitDialog(user, fetchMock, 1);
    expect(postedRuleBodies(fetchMock)[0].transformations).toBe("trim");

    // ── Edición: el campo llega precargado con lo almacenado y viaja intacto ──
    cleanup();
    renderUpdateDialog({ ...WAF_ITEM, transformations: "urlDecodeUni" });
    await openRequestTab(user);
    expect(transformationsInput().value).toBe("urlDecodeUni");

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));
    expect(patchedRuleBodies(fetchMock)[0].transformations).toBe("urlDecodeUni");
  });
});

// ─── Los chips: nombre accesible del grupo, saneo del borde y contrato vacío ──
// El grupo de dieciocho chips necesita nombre accesible propio: sin él, quien
// use un lector de pantalla oye botones sueltos sin saber a qué conjunto
// pertenecen.
//
// El chip **añade** al final de lo escrito a mano, así que el borde derecho de
// lo que ya había se sanea antes de concatenar: si el operador dejó una coma o
// un espacio de sobra, el token no puede entrar tras un separador vacío. El
// parser del backend es estricto y un token vacío (`unknown transformation
// token ''`) devuelve un `400` que no orienta nada.
//
// La repetición deliberada sigue permitida: `urlDecodeUni,urlDecodeUni` es como
// se cubre una doble codificación, así que ningún `Set` debe colarse aquí.
describe("RuleDialog: el grupo de chips de transformaciones", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  /** Sustituye `fetch` por un doble que responde 200 con un cuerpo vacío. */
  function stubFetch(): FetchStub {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({}),
    });
    vi.stubGlobal("fetch", fetchMock);
    return fetchMock;
  }

  /** Pulsa la pestaña «Request», que es la única que monta el campo y los chips. */
  async function openRequestTab(user: UserEvent): Promise<void> {
    await user.click(await screen.findByRole("tab", { name: /request/i }));
  }

  /** El campo de texto de la especificación, por su `aria-label`. */
  function transformationsInput(): HTMLInputElement {
    return screen.getByRole("textbox", {
      name: "Transformations",
    }) as HTMLInputElement;
  }

  it("exposes the chip container as a named group", async () => {
    const user = userEvent.setup();
    stubFetch();

    renderCreateDialog();
    await openRequestTab(user);

    // El conjunto de chips tiene rol `group` y nombre accesible propio, de modo
    // que se distingue del campo de texto (mismo nombre, rol distinto).
    expect(
      screen.getByRole("group", { name: "Transformations" }),
    ).toBeTruthy();
  });

  it("sanitizes the right edge of hand-written text before appending a chip", async () => {
    const user = userEvent.setup();
    stubFetch();

    renderCreateDialog();
    await openRequestTab(user);

    // ── Coma final: `trim,` + chip → `trim,lowercase`, nunca `trim,,lowercase` ──
    await user.type(transformationsInput(), "trim,");
    await user.click(screen.getByRole("button", { name: "lowercase" }));
    expect(transformationsInput().value).toBe("trim,lowercase");

    // ── Espacio final: `trim ` + chip → `trim,lowercase`, nunca `trim ,lowercase` ──
    await user.clear(transformationsInput());
    await user.type(transformationsInput(), "trim ");
    await user.click(screen.getByRole("button", { name: "lowercase" }));
    expect(transformationsInput().value).toBe("trim,lowercase");
  });

  it("allows repeating a token on purpose: pressing urlDecodeUni twice writes it twice", async () => {
    const user = userEvent.setup();
    stubFetch();

    renderCreateDialog();
    await openRequestTab(user);

    // Repetir un token es una elección legítima —la pipeline se aplica una sola
    // vez y una doble codificación se cubre repitiendo el decodificador—, así
    // que el chip no deduplica.
    await user.click(screen.getByRole("button", { name: "urlDecodeUni" }));
    await user.click(screen.getByRole("button", { name: "urlDecodeUni" }));
    expect(transformationsInput().value).toBe("urlDecodeUni,urlDecodeUni");
  });

  it("sends an explicitly empty transformations field when editing a rule with none", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog({ ...WAF_ITEM, transformations: "" });
    await openRequestTab(user);
    expect(transformationsInput().value).toBe("");

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));
    const body = patchedRuleBodies(fetchMock)[0];
    // La clave viaja **presente** y vacía, no ausente: la cadena vacía es la
    // pipeline vacía y el `PATCH` declara el campo como obligatorio.
    expect(Object.keys(body)).toContain("transformations");
    expect(body.transformations).toBe("");
  });
});
