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
