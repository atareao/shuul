import { describe, it, expect, vi, beforeAll, afterEach } from "vitest";
import {
  render,
  screen,
  within,
  waitFor,
  cleanup,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { UserEvent } from "@testing-library/user-event";
import RuleDialog from "./rule_dialog";
import type Item from "@/models/rule";
import { DialogModes } from "@/common/types";

// El diálogo compone sus URLs con `BASE_URL`; se sustituye por un valor fijo
// para que el test no dependa del entorno (mismo habilitador que
// `rule_dialog.test.tsx`).
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
// Escenarios del delta `rule-conditions-editor`: el diálogo de regla gana una
// pestaña «Conditions» con un conmutador entre modo simple y avanzado. En simple
// omite `conditions` (el backend proyecta los 14 campos); en avanzado envía la
// lista —aunque esté vacía—, deshabilita las pestañas de los 14 campos y exige
// clave a las colecciones que la necesitan. Al editar, el modo se infiere
// comparando las condiciones recibidas con la proyección de los 14 campos.
//
// El `t` que recibe el diálogo es `(key) => key`, así que toda etiqueta visible
// y todo nombre accesible valen literalmente la clave de traducción.

/** Forma serializada exacta de una condición: la misma en lectura y escritura. */
interface ApiCondition {
  collection: string;
  key: string | null;
  transformations: string;
  operator: string;
  pattern: string;
}

/** Cuerpo que el diálogo envía a `POST`/`PATCH /api/v1/rules`. */
interface RuleBody {
  pipeline: string;
  weight: number;
  transformations: string;
  conditions?: ApiCondition[];
  [key: string]: unknown;
}

/** Forma mínima del `fetch` sustituido: solo importan las opciones de cada
 *  llamada (el segundo argumento), que es donde viajan método y cuerpo. */
interface FetchStub {
  mock: { calls: RequestInit[][] };
}

/** Los 14 campos simples, tal y como viajan en el cuerpo de escritura. */
const RULE_FIELDS = [
  "ip_address",
  "protocol",
  "fqdn",
  "path",
  "query",
  "city_name",
  "country_name",
  "country_code",
  "user_agent",
  "method",
  "referer",
  "content_type",
  "accept_language",
  "x_request_id",
] as const;

/**
 * La nota que se muestra en modo avanzado, copiada literal de la clave de
 * traducción: con `t` identidad, el texto visible es exactamente este.
 */
const ADVANCED_NOTE =
  "In advanced mode these fields are stored but do not participate in matching";

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

/** Abre el diálogo de regla en modo creación. */
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
 * Pulsa la pestaña «Conditions».
 *
 * antd sólo monta el panel de la pestaña activa, así que el conmutador de modo y
 * la lista de condiciones no existen en el DOM hasta pulsarla. La etiqueta lleva
 * icono en las demás pestañas (el icono aporta su `aria-label` al nombre
 * accesible), así que el `tab` se localiza con una expresión regular.
 */
async function openConditionsTab(user: UserEvent): Promise<void> {
  await user.click(await screen.findByRole("tab", { name: /conditions/i }));
}

/**
 * El conmutador de modo.
 *
 * antd rinde `Segmented` como un `radiogroup` con un `input[type=radio]`
 * nativo por opción, cada uno dentro de su `label`. El estilo de antd pone
 * `pointer-events: none` en el input, así que sólo la etiqueta visible recibe
 * el puntero; para cambiarlo se pulsa la etiqueta con user-event
 * (`user.click(screen.getByText("Advanced"))`) —verificado empíricamente—.
 * Para **comprobar el estado** el `getByRole("radio", { name })` sigue
 * valiendo: el nombre accesible del `radio` es el texto visible de la opción.
 */
function simpleRadio(): HTMLElement {
  return screen.getByRole("radio", { name: "Simple" });
}

function advancedRadio(): HTMLElement {
  return screen.getByRole("radio", { name: "Advanced" });
}

/** Cambia a modo avanzado pulsando la etiqueta visible de la opción. */
async function switchToAdvanced(user: UserEvent): Promise<void> {
  await user.click(screen.getByText("Advanced"));
}

/** La lista de condiciones: contenedor con rol `list` y nombre accesible. */
function conditionsList(): HTMLElement {
  return screen.getByRole("list", { name: "Conditions" });
}

/** Las filas de la lista de condiciones. */
function conditionRows(): HTMLElement[] {
  return within(conditionsList()).queryAllByRole("listitem");
}

/** El valor del Input de patrón de una fila. */
function patternOf(row: HTMLElement): string {
  return (
    within(row).getByRole("textbox", { name: "Pattern" }) as HTMLInputElement
  ).value;
}

/**
 * Confirma el aviso de vuelta al modo simple.
 *
 * La implementación usa un `Popconfirm` controlado, que rinde su superficie
 * como `.ant-popover:not(.ant-popover-hidden)` —en árbol, así que se desmonta
 * con el componente y no se filtra entre tests—. El botón primario se localiza
 * **dentro de ese contenedor** para no confundirlo con el `Ok` del propio
 * diálogo de regla; su nombre accesible es `Ok` (el texto lo pasa `t`), así que
 * la expresión regular acepta además cualquier variante de antd (`OK`).
 */
async function confirmModeSwitch(user: UserEvent): Promise<void> {
  const surface = await waitFor(() => {
    const found =
      document.querySelector<HTMLElement>(
        '[role="dialog"].ant-modal-confirm',
      ) ??
      document.querySelector<HTMLElement>(
        ".ant-popover:not(.ant-popover-hidden)",
      );
    if (!found) {
      throw new Error("Mode-switch confirmation surface not found");
    }
    return found;
  });
  await user.click(within(surface).getByRole("button", { name: /^ok$/i }));
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

/** Regla de WAF ya almacenada, base de las fixtures de edición. */
const WAF_ITEM: Item = {
  id: 8,
  name: "WAF rule",
  description: "",
  mode: "enforce",
  pipeline: "waf",
  weight: 250,
};

/**
 * Regla que la API devuelve en modo simple: sus condiciones son exactamente la
 * proyección de su único campo relleno, así que el modo inferido es simple.
 */
const SIMPLE_ITEM: Item = {
  ...WAF_ITEM,
  transformations: "",
  path: "/admin",
  conditions: [
    {
      collection: "path",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "/admin",
    },
  ],
};

/**
 * Regla que la API devuelve en modo avanzado: su condición con clave no es la
 * proyección de sus 14 campos (ninguno está relleno), así que difiere y el modo
 * inferido es avanzado.
 */
const ADVANCED_ITEM: Item = {
  ...WAF_ITEM,
  transformations: "",
  conditions: [
    {
      collection: "request_cookies",
      key: "PHPSESSID",
      transformations: "",
      operator: "@rx",
      pattern: ".+",
    },
  ],
};

/**
 * Regla en modo simple que además restringe Tor: sus condiciones son la
 * proyección de su único campo relleno **más** la condición `is_tor`/`@truthy`
 * que proyecta la columna. Es la prueba de que una regla simple con `is_tor`
 * **no** se degrada a avanzado.
 */
const SIMPLE_TOR_ITEM: Item = {
  ...WAF_ITEM,
  transformations: "",
  path: "/admin",
  is_tor: true,
  conditions: [
    {
      collection: "path",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "/admin",
    },
    {
      collection: "is_tor",
      key: null,
      transformations: "",
      operator: "@truthy",
      pattern: "",
    },
  ],
};

/**
 * Regla en modo avanzado cuyo conjunto persistido es una única condición
 * `is_tor`/`@truthy`: la columna `is_tor` es falsa y no participa, así que la
 * lista difiere de la proyección de los 14 campos y abre en avanzado con esa
 * fila.
 */
const TOR_CONDITION_ITEM: Item = {
  ...WAF_ITEM,
  transformations: "",
  is_tor: false,
  conditions: [
    {
      collection: "is_tor",
      key: null,
      transformations: "",
      operator: "@truthy",
      pattern: "",
    },
  ],
};

describe("RuleDialog: el conmutador de modo simple y avanzado", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("abre una regla nueva en modo simple y, al guardar, omite `conditions` pero envía los 14 campos", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderCreateDialog();
    await openConditionsTab(user);

    // Una regla nueva abre en simple.
    expect(simpleRadio()).toBeChecked();

    await submitDialog(user, fetchMock, 1);
    const body = postedRuleBodies(fetchMock)[0];

    // El contrato de modo: en simple el diálogo **omite** la clave por completo
    // —el backend proyecta los 14 campos—, y los 14 campos siguen viajando.
    expect(Object.keys(body)).not.toContain("conditions");
    for (const field of RULE_FIELDS) {
      expect(Object.keys(body)).toContain(field);
    }
  });

  it("sembrar al pasar a avanzado: proyecta los 14 campos rellenos en orden canónico", async () => {
    const user = userEvent.setup();
    stubFetch();

    renderCreateDialog();

    // Se rellenan los campos simples en la pestaña «Request», a propósito en
    // orden inverso al canónico: `user_agent` va después de `path` en la lista.
    await user.click(await screen.findByRole("tab", { name: /request/i }));
    await user.type(screen.getByPlaceholderText("User Agent"), "curl/8.0");
    await user.type(screen.getByPlaceholderText("Path"), "/admin");

    await openConditionsTab(user);
    await switchToAdvanced(user);

    // Una condición por cada campo relleno, con su colección y su patrón.
    const rows = conditionRows();
    expect(rows).toHaveLength(2);
    expect(within(rows[0]).getByText("path")).toBeInTheDocument();
    expect(patternOf(rows[0])).toBe("/admin");
    expect(within(rows[1]).getByText("user_agent")).toBeInTheDocument();
    expect(patternOf(rows[1])).toBe("curl/8.0");
  });

  it("en avanzado envía `conditions` —la lista vacía incluida— y los 14 campos siguen viajando", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    // ── Lista vacía: la clave viaja **presente** y vacía, no ausente ──
    renderCreateDialog();
    await openConditionsTab(user);
    await switchToAdvanced(user);

    expect(conditionRows()).toHaveLength(0);

    await submitDialog(user, fetchMock, 1);
    const emptyBody = postedRuleBodies(fetchMock)[0];
    expect(Object.keys(emptyBody)).toContain("conditions");
    expect(emptyBody.conditions).toEqual([]);
    // Los 14 campos se siguen enviando: en avanzado se almacenan pero no matchean.
    for (const field of RULE_FIELDS) {
      expect(Object.keys(emptyBody)).toContain(field);
    }

    // ── Con condiciones: viajan en orden, con la forma serializada exacta ──
    cleanup();
    renderCreateDialog();
    await user.click(await screen.findByRole("tab", { name: /request/i }));
    await user.type(screen.getByPlaceholderText("User Agent"), "curl/8.0");
    await user.type(screen.getByPlaceholderText("Path"), "/admin");
    await openConditionsTab(user);
    await switchToAdvanced(user);

    await submitDialog(user, fetchMock, 2);
    const body = postedRuleBodies(fetchMock)[1];
    expect(Object.keys(body)).toContain("conditions");
    expect(body.conditions).toEqual([
      {
        collection: "path",
        key: null,
        transformations: "",
        operator: "@rx",
        pattern: "/admin",
      },
      {
        collection: "user_agent",
        key: null,
        transformations: "",
        operator: "@rx",
        pattern: "curl/8.0",
      },
    ]);
  });

  it("el botón `Add condition` añade una fila vacía y el botón `Remove` la retira", async () => {
    const user = userEvent.setup();
    stubFetch();

    renderCreateDialog();
    await openConditionsTab(user);
    await switchToAdvanced(user);

    expect(within(conditionsList()).queryAllByRole("listitem")).toHaveLength(0);

    await user.click(screen.getByRole("button", { name: "Add condition" }));
    expect(within(conditionsList()).queryAllByRole("listitem")).toHaveLength(1);

    await user.click(
      within(conditionsList()).getByRole("button", { name: "Remove" }),
    );
    expect(within(conditionsList()).queryAllByRole("listitem")).toHaveLength(0);
  });

  it("en avanzado deshabilita Network, Location y Request y muestra la nota", async () => {
    const user = userEvent.setup();
    stubFetch();

    renderCreateDialog();
    await openConditionsTab(user);
    await switchToAdvanced(user);

    // Las pestañas de los 14 campos quedan deshabilitadas...
    for (const name of [/network/i, /location/i, /request/i]) {
      expect(screen.getByRole("tab", { name }).getAttribute("aria-disabled")).toBe(
        "true",
      );
    }
    // ...mientras «General» y «Conditions» siguen operativas.
    expect(
      screen.getByRole("tab", { name: /general/i }).getAttribute("aria-disabled"),
    ).not.toBe("true");
    expect(
      screen
        .getByRole("tab", { name: /conditions/i })
        .getAttribute("aria-disabled"),
    ).not.toBe("true");

    expect(screen.getByText(ADVANCED_NOTE)).toBeTruthy();
  });

  it("volver a simple pide confirmación y, al confirmar, guarda sin `conditions`", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderCreateDialog();
    await openConditionsTab(user);
    await switchToAdvanced(user);
    await user.click(screen.getByRole("button", { name: "Add condition" }));
    expect(screen.getByText(ADVANCED_NOTE)).toBeTruthy();

    // Se pide confirmación: sin confirmar, el modo no cambia.
    await user.click(screen.getByText("Simple"));
    expect(screen.getByText(ADVANCED_NOTE)).toBeTruthy();
    expect(advancedRadio()).toBeChecked();

    await confirmModeSwitch(user);
    await waitFor(() => expect(screen.queryByText(ADVANCED_NOTE)).toBeNull());
    expect(simpleRadio()).toBeChecked();

    // La lista se descarta y el cuerpo vuelve a omitir `conditions`.
    expect(screen.queryByRole("list", { name: "Conditions" })).toBeNull();
    await submitDialog(user, fetchMock, 1);
    expect(Object.keys(postedRuleBodies(fetchMock)[0])).not.toContain(
      "conditions",
    );
  });
});

// ─── La inferencia del modo al editar una regla existente ────────────────────
// La API devuelve el conjunto **efectivo**: en modo simple, la proyección de los
// 14 campos. El diálogo infiere el modo comparando las condiciones recibidas con
// esa proyección —campo a campo, con `key ?? null` y `operator ?? "@rx"`—, de
// modo que la inferencia es sin pérdida: al volver a enviar en simple se
// reproduce el mismo conjunto.
describe("RuleDialog: la inferencia del modo al editar", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("una regla cuyas condiciones son la proyección de sus campos abre en simple y guarda sin `conditions`", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog(SIMPLE_ITEM);
    await openConditionsTab(user);

    expect(simpleRadio()).toBeChecked();
    expect(screen.queryByText(ADVANCED_NOTE)).toBeNull();
    expect(
      screen.getByRole("tab", { name: /request/i }).getAttribute("aria-disabled"),
    ).not.toBe("true");

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));
    expect(Object.keys(patchedRuleBodies(fetchMock)[0])).not.toContain(
      "conditions",
    );
  });

  it("una regla cuyas condiciones difieren de la proyección abre en avanzado con sus condiciones y las reenvía", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog(ADVANCED_ITEM);
    await openConditionsTab(user);

    expect(advancedRadio()).toBeChecked();
    expect(screen.getByText(ADVANCED_NOTE)).toBeTruthy();
    expect(
      screen.getByRole("tab", { name: /request/i }).getAttribute("aria-disabled"),
    ).toBe("true");

    // La lista se siembra con las condiciones recibidas, con la clave incluida
    // porque `request_cookies` la exige, y el operador fijo a la vista.
    const rows = conditionRows();
    expect(rows).toHaveLength(1);
    expect(within(rows[0]).getByText("request_cookies")).toBeInTheDocument();
    expect(
      (
        within(rows[0]).getByRole("textbox", {
          name: "Key",
        }) as HTMLInputElement
      ).value,
    ).toBe("PHPSESSID");
    expect(patternOf(rows[0])).toBe(".+");
    expect(within(rows[0]).getByText("@rx")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));
    const body = patchedRuleBodies(fetchMock)[0];
    expect(Object.keys(body)).toContain("conditions");
    expect(body.conditions).toEqual([
      {
        collection: "request_cookies",
        key: "PHPSESSID",
        transformations: "",
        operator: "@rx",
        pattern: ".+",
      },
    ]);
  });

  it("una regla simple con `is_tor` abre en simple: no degrada a avanzado", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog(SIMPLE_TOR_ITEM);
    await openConditionsTab(user);

    // Sus condiciones son la proyección de sus 14 campos **más** la condición
    // `is_tor`/`@truthy` que proyecta la columna, así que el modo inferido es
    // simple: la proyección las reproduce exactamente.
    expect(simpleRadio()).toBeChecked();
    expect(screen.queryByText(ADVANCED_NOTE)).toBeNull();

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));
    expect(Object.keys(patchedRuleBodies(fetchMock)[0])).not.toContain(
      "conditions",
    );
  });
});

// ─── La validación de la clave: una colección que la exige no puede ir vacía ──
// Escenario del delta: «Una colección con clave sin clave impide guardar».
// La señalización exigible es el `aria-invalid="true"` del Input de la clave de
// la fila pendiente: es accesible —lo anuncia el lector de pantalla— y estable
// de consultar. Se descartó `role="alert"` con mensaje por ser otra forma de la
// misma señal; basta una.
describe("RuleDialog: una colección con clave sin clave impide guardar", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("no registra ninguna escritura y señala la fila pendiente", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    // La regla ya trae una condición de `request_cookies` con clave; se vacía.
    renderUpdateDialog(ADVANCED_ITEM);
    await openConditionsTab(user);

    const rows = conditionRows();
    expect(rows).toHaveLength(1);
    const keyInput = within(rows[0]).getByRole("textbox", {
      name: "Key",
    }) as HTMLInputElement;
    await user.clear(keyInput);
    expect(keyInput.value).toBe("");

    await user.click(screen.getByRole("button", { name: "Ok" }));

    // El diálogo impide guardar: no viaja ninguna petición de escritura...
    await waitFor(() => expect(keyInput).toHaveAttribute("aria-invalid", "true"));
    expect(patchedRuleBodies(fetchMock)).toHaveLength(0);
    expect(postedRuleBodies(fetchMock)).toHaveLength(0);
  });
});

// ─── El operador fijo de `is_tor`: `@truthy`, sin patrón, clave ni pipeline ───
// Escenario del delta: «El operador se fija para `is_tor`». La colección booleana
// no se compara contra un patrón ni se resuelve por nombre, así que su fila fija
// `@truthy` y **no** ofrece patrón, clave ni transformaciones.
describe("RuleDialog: el operador fijo de `is_tor`", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("fija `@truthy` y no ofrece patrón, clave ni transformaciones en la fila `is_tor`", async () => {
    const user = userEvent.setup();
    stubFetch();

    renderUpdateDialog(TOR_CONDITION_ITEM);
    await openConditionsTab(user);

    const rows = conditionRows();
    expect(rows).toHaveLength(1);
    const row = rows[0];

    // El operador está fijado a `@truthy` y a la vista.
    expect(within(row).getByText("@truthy")).toBeInTheDocument();

    // Ni patrón, ni clave, ni transformaciones: no tiene sentido para un booleano.
    expect(within(row).queryByRole("textbox", { name: "Pattern" })).toBeNull();
    expect(within(row).queryByRole("textbox", { name: "Key" })).toBeNull();
    expect(
      within(row).queryByRole("textbox", { name: "Transformations" }),
    ).toBeNull();
    expect(
      within(row).queryByRole("group", { name: "Transformations" }),
    ).toBeNull();
  });

  it("al guardar envía la condición `is_tor` serializada como `@truthy`, sin patrón, clave ni transformaciones", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog(TOR_CONDITION_ITEM);
    await openConditionsTab(user);

    expect(conditionRows()).toHaveLength(1);

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));

    // El contrato de serialización: la rama `is_tor` de `serializeCondition`
    // fija el operador a `@truthy` y deja patrón y transformaciones vacíos, con
    // la clave presente y nula —la forma exacta que espera la API—.
    const body = patchedRuleBodies(fetchMock)[0];
    expect(Object.keys(body)).toContain("conditions");
    expect(body.conditions).toEqual([
      {
        collection: "is_tor",
        key: null,
        transformations: "",
        operator: "@truthy",
        pattern: "",
      },
    ]);
  });
});

// ─── El operador por fila: el catálogo, el patrón oculto y la validación ─────
// Escenarios del delta `waf-string-operators` (F2c-1): la autoría avanzada elige
// el operador de **cada fila** entre los siete del catálogo —`@rx` por defecto
// más los seis operadores de cadena de Coraza—. `is_tor` conserva `@truthy`
// **fijo** y sin selector; `@unconditionalMatch` **oculta** el campo de patrón y
// lo envía vacío; y la validación de fila impide guardar mientras alguna fila
// tenga el patrón vacío **salvo** en `@unconditionalMatch`.
//
// Localización de los controles: el `Select` de antd expone el `aria-label` en su
// `combobox` —verificado empíricamente—, así que el selector de operador de una
// fila se localiza por su nombre accesible `Operator` **dentro de esa fila**, y
// sus opciones se consultan por el `title` de la opción del desplegable.

/**
 * El desplegable abierto del `Select` de operador de la fila.
 *
 * El desplegable vive en un portal fuera de la fila, así que no se puede acotar
 * con `within(row)`. antd lo asocia al `combobox` por `aria-controls`, que apunta
 * al `id` del `listbox` de **ese** selector: es la forma de elegir el de esta
 * fila aunque otra tenga el suyo montado —tras abrirse una vez, antd deja el
 * `listbox` en el DOM— y evita la consulta global ambigua a `listbox`. Si el
 * atributo no estuviese, se degrada a la búsqueda global.
 */
async function openOperatorDropdown(
  user: UserEvent,
  row: HTMLElement,
): Promise<HTMLElement> {
  const combobox = within(row).getByRole("combobox", { name: "Operator" });
  await user.click(combobox);
  const listboxId = combobox.getAttribute("aria-controls");
  const listbox = listboxId
    ? await waitFor(() => {
        const found = document.getElementById(listboxId);
        if (!found) {
          throw new Error(`Operator listbox ${listboxId} not found`);
        }
        return found;
      })
    : await screen.findByRole("listbox");
  const dropdown = listbox.closest<HTMLElement>(".ant-select-dropdown");
  if (!dropdown) {
    throw new Error("Operator dropdown not found");
  }
  return dropdown;
}

/** Elige la opción `name` en el selector de operador de la fila. */
async function chooseOperator(
  user: UserEvent,
  row: HTMLElement,
  name: string,
): Promise<void> {
  const dropdown = await openOperatorDropdown(user, row);
  await user.click(within(dropdown).getByTitle(name));
}

/**
 * Regla en modo avanzado con dos filas: la booleana `is_tor`/`@truthy` y una de
 * texto `method`/`@rx`.
 *
 * Las dos juntas hacen visible el contrato del operador por fila: el selector se
 * ofrece en la de texto —con su operador a la vista— y **no** en la booleana,
 * cuyo operador está fijo. La columna `is_tor` es falsa y no participa, así que
 * la lista difiere de la proyección de los 14 campos —ninguno relleno— y el
 * diálogo abre en avanzado.
 */
const TOR_AND_TEXT_ITEM: Item = {
  ...WAF_ITEM,
  transformations: "",
  is_tor: false,
  conditions: [
    {
      collection: "is_tor",
      key: null,
      transformations: "",
      operator: "@truthy",
      pattern: "",
    },
    {
      collection: "method",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "POST",
    },
  ],
};

/**
 * Regla cuyos 14 campos están rellenos y cuyas condiciones son **exactamente** su
 * proyección: catorce condiciones `@rx` en orden canónico, sin `is_tor`. La
 * inferencia del modo debe reconocerla como simple y no degradarla a avanzado.
 */
const FULL_SIMPLE_ITEM: Item = {
  ...WAF_ITEM,
  transformations: "",
  is_tor: false,
  ip_address: "10.0.0.1",
  protocol: "http",
  fqdn: "example.com",
  path: "/admin",
  query: "a=1",
  city_name: "Madrid",
  country_name: "Spain",
  country_code: "ES",
  user_agent: "curl/8.0",
  method: "POST",
  referer: "https://example.com/",
  content_type: "application/json",
  accept_language: "es-ES",
  x_request_id: "abc-123",
  conditions: [
    {
      collection: "ip_address",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "10.0.0.1",
    },
    {
      collection: "protocol",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "http",
    },
    {
      collection: "fqdn",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "example.com",
    },
    {
      collection: "path",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "/admin",
    },
    {
      collection: "query",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "a=1",
    },
    {
      collection: "city_name",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "Madrid",
    },
    {
      collection: "country_name",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "Spain",
    },
    {
      collection: "country_code",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "ES",
    },
    {
      collection: "user_agent",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "curl/8.0",
    },
    {
      collection: "method",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "POST",
    },
    {
      collection: "referer",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "https://example.com/",
    },
    {
      collection: "content_type",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "application/json",
    },
    {
      collection: "accept_language",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "es-ES",
    },
    {
      collection: "x_request_id",
      key: null,
      transformations: "",
      operator: "@rx",
      pattern: "abc-123",
    },
  ],
};

/**
 * Regla en modo avanzado cuyo conjunto persistido es una única condición
 * `method`/`@streq`/`POST`: un operador de cadena que la proyección de los 14
 * campos **no** produce —`method` está vacío en la columna—, así que la lista
 * difiere y el diálogo abre en avanzado con esa fila y su operador.
 */
const STREQ_ITEM: Item = {
  ...WAF_ITEM,
  transformations: "",
  method: "",
  conditions: [
    {
      collection: "method",
      key: null,
      transformations: "",
      operator: "@streq",
      pattern: "POST",
    },
  ],
};

describe("RuleDialog: el operador por fila", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("el operador recibido se refleja en el selector, que ofrece los siete del catálogo", async () => {
    const user = userEvent.setup();
    stubFetch();

    renderUpdateDialog(ADVANCED_ITEM);
    await openConditionsTab(user);

    const rows = conditionRows();
    expect(rows).toHaveLength(1);

    // El operador recibido (`@rx`) es el valor elegido en el selector.
    expect(within(rows[0]).getByText("@rx")).toBeInTheDocument();

    const dropdown = await openOperatorDropdown(user, rows[0]);
    expect(
      within(dropdown)
        .getAllByRole("option")
        .map((option) => option.textContent),
    ).toEqual([
      "@rx",
      "@contains",
      "@streq",
      "@beginsWith",
      "@endsWith",
      "@within",
      "@unconditionalMatch",
    ]);
  });

  it("una fila nueva trae `@rx` como operador por defecto", async () => {
    const user = userEvent.setup();
    stubFetch();

    // Se parte de modo avanzado vacío y se añade la fila, que nace sin operador
    // recibido: el suyo es el valor por defecto del catálogo, no uno copiado.
    renderCreateDialog();
    await openConditionsTab(user);
    await switchToAdvanced(user);
    expect(conditionRows()).toHaveLength(0);

    await user.click(screen.getByRole("button", { name: "Add condition" }));

    const rows = conditionRows();
    expect(rows).toHaveLength(1);
    // La fila nueva ofrece su selector de operador y éste trae `@rx` elegido.
    expect(
      within(rows[0]).getByRole("combobox", { name: "Operator" }),
    ).toBeInTheDocument();
    expect(within(rows[0]).getByText("@rx")).toBeInTheDocument();
  });

  it("`@unconditionalMatch` oculta el campo de patrón y lo envía vacío", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog(ADVANCED_ITEM);
    await openConditionsTab(user);

    const rows = conditionRows();
    expect(patternOf(rows[0])).toBe(".+");

    await chooseOperator(user, rows[0], "@unconditionalMatch");

    // El campo de patrón desaparece: dejar el anterior en la fila produciría un
    // `400` al guardar, porque el operador exige el patrón vacío.
    await waitFor(() =>
      expect(
        within(conditionRows()[0]).queryByRole("textbox", { name: "Pattern" }),
      ).toBeNull(),
    );

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));

    expect(patchedRuleBodies(fetchMock)[0].conditions).toEqual([
      {
        collection: "request_cookies",
        key: "PHPSESSID",
        transformations: "",
        operator: "@unconditionalMatch",
        pattern: "",
      },
    ]);
  });

  it("la fila `is_tor` no ofrece selector de operador y envía `@truthy` sin patrón", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog(TOR_AND_TEXT_ITEM);
    await openConditionsTab(user);

    const rows = conditionRows();
    expect(rows).toHaveLength(2);
    const [torRow, textRow] = rows;

    // La booleana conserva `@truthy` **fijo** y sin selector: ofrecerlo insinuaría
    // una elección que la validación rechaza.
    expect(
      within(torRow).queryByRole("combobox", { name: "Operator" }),
    ).toBeNull();
    expect(within(torRow).getByText("@truthy")).toBeInTheDocument();

    // La fila de texto, en cambio, sí ofrece el suyo.
    expect(
      within(textRow).getByRole("combobox", { name: "Operator" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));

    expect(patchedRuleBodies(fetchMock)[0].conditions).toEqual([
      {
        collection: "is_tor",
        key: null,
        transformations: "",
        operator: "@truthy",
        pattern: "",
      },
      {
        collection: "method",
        key: null,
        transformations: "",
        operator: "@rx",
        pattern: "POST",
      },
    ]);
  });

  it("una fila con el patrón vacío impide guardar salvo en `@unconditionalMatch`", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    // ── Patrón vacío con `@rx`: la fila queda pendiente y no se emite nada ──
    renderUpdateDialog(ADVANCED_ITEM);
    await openConditionsTab(user);

    await user.clear(
      within(conditionRows()[0]).getByRole("textbox", { name: "Pattern" }),
    );
    await user.click(screen.getByRole("button", { name: "Ok" }));

    // La fila pendiente se señala con un mensaje accesible —la clave de
    // traducción literal, porque `t` es la identidad—, igual que la clave que
    // falta en una colección con clave.
    await waitFor(() =>
      expect(within(conditionRows()[0]).getByRole("alert")).toHaveTextContent(
        "This pattern is required",
      ),
    );
    // Y no viaja ninguna petición de escritura: el cuerpo sería inválido.
    expect(patchedRuleBodies(fetchMock)).toHaveLength(0);
    expect(postedRuleBodies(fetchMock)).toHaveLength(0);

    // ── Patrón vacío con `@unconditionalMatch`: es su contrato y sí guarda ──
    cleanup();
    renderUpdateDialog(ADVANCED_ITEM);
    await openConditionsTab(user);
    await chooseOperator(user, conditionRows()[0], "@unconditionalMatch");

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));
    expect(patchedRuleBodies(fetchMock)[0].conditions).toEqual([
      {
        collection: "request_cookies",
        key: "PHPSESSID",
        transformations: "",
        operator: "@unconditionalMatch",
        pattern: "",
      },
    ]);
  });

  it("`@contains` viaja con su operador y con el patrón escrito tal cual", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog(ADVANCED_ITEM);
    await openConditionsTab(user);

    await chooseOperator(user, conditionRows()[0], "@contains");

    const patternInput = within(conditionRows()[0]).getByRole("textbox", {
      name: "Pattern",
    });
    await user.clear(patternInput);
    await user.type(patternInput, "nikto");

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));

    expect(patchedRuleBodies(fetchMock)[0].conditions).toEqual([
      {
        collection: "request_cookies",
        key: "PHPSESSID",
        transformations: "",
        operator: "@contains",
        pattern: "nikto",
      },
    ]);
  });

  it("`@within` conserva el campo de patrón y envía la lista separada por comas tal cual", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog(ADVANCED_ITEM);
    await openConditionsTab(user);

    await chooseOperator(user, conditionRows()[0], "@within");

    // El contrato visible de `@within`: el campo de patrón **sigue** visible
    // —sólo `@unconditionalMatch` lo oculta— y en él se escribe la lista
    // separada por comas, que viaja tal cual, sin recortar.
    const patternInput = within(conditionRows()[0]).getByRole("textbox", {
      name: "Pattern",
    });
    await user.clear(patternInput);
    await user.type(patternInput, "GET, POST");

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));

    expect(patchedRuleBodies(fetchMock)[0].conditions).toEqual([
      {
        collection: "request_cookies",
        key: "PHPSESSID",
        transformations: "",
        operator: "@within",
        pattern: "GET, POST",
      },
    ]);
  });

  it("`@beginsWith` conserva el campo de patrón y envía su patrón literal", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog(ADVANCED_ITEM);
    await openConditionsTab(user);

    await chooseOperator(user, conditionRows()[0], "@beginsWith");

    const patternInput = within(conditionRows()[0]).getByRole("textbox", {
      name: "Pattern",
    });
    await user.clear(patternInput);
    await user.type(patternInput, "/admin");

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));

    expect(patchedRuleBodies(fetchMock)[0].conditions).toEqual([
      {
        collection: "request_cookies",
        key: "PHPSESSID",
        transformations: "",
        operator: "@beginsWith",
        pattern: "/admin",
      },
    ]);
  });
});

// ─── La inferencia del modo no cambia con los operadores de cadena ────────────
// La inferencia sigue comparando las condiciones recibidas con la proyección de
// los 14 campos y de la columna `is_tor`, que sólo produce `@rx` y `@truthy`. Una
// regla simple no se degrada, y una avanzada con un operador de cadena —que la
// proyección no puede producir— abre en avanzado con su lista y su operador.
describe("RuleDialog: la inferencia del modo con operadores de cadena", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("una regla cuyas condiciones son la proyección de los 14 campos abre en simple", async () => {
    const user = userEvent.setup();
    const fetchMock = stubFetch();

    renderUpdateDialog(FULL_SIMPLE_ITEM);
    await openConditionsTab(user);

    expect(simpleRadio()).toBeChecked();
    expect(screen.queryByText(ADVANCED_NOTE)).toBeNull();
    expect(screen.queryByRole("list", { name: "Conditions" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "Ok" }));
    await waitFor(() => expect(patchedRuleBodies(fetchMock)).toHaveLength(1));
    expect(Object.keys(patchedRuleBodies(fetchMock)[0])).not.toContain(
      "conditions",
    );
  });

  it("una regla con un operador de cadena abre en avanzado con su fila y su operador a la vista", async () => {
    const user = userEvent.setup();
    stubFetch();

    renderUpdateDialog(STREQ_ITEM);
    await openConditionsTab(user);

    expect(advancedRadio()).toBeChecked();
    expect(screen.getByText(ADVANCED_NOTE)).toBeTruthy();

    const rows = conditionRows();
    expect(rows).toHaveLength(1);
    expect(within(rows[0]).getByText("method")).toBeInTheDocument();
    expect(patternOf(rows[0])).toBe("POST");

    // La lista se siembra con el operador recibido, que queda elegido en el
    // selector de la fila: la inferencia no lo degrada a `@rx`.
    expect(
      within(rows[0]).getByRole("combobox", { name: "Operator" }),
    ).toBeInTheDocument();
    expect(within(rows[0]).getByText("@streq")).toBeInTheDocument();
  });
});
