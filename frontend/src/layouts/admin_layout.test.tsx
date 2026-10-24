import {
  describe,
  it,
  expect,
  vi,
  beforeAll,
  beforeEach,
  afterEach,
} from "vitest";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Routes, Route } from "react-router";

import AdminLayout from "./admin_layout";
import AuthContext from "@/components/auth_context";
import type { AuthContextInterface } from "@/components/auth_context";
import { ModeContextProvider } from "@/components/mode_context";

// ─────────────────────────────────────────────────────────────────────────────
// Setup de entorno de navegador mínimo para antd (jsdom carece de estas APIs).
// Mismo stub que monta `pages/admin/templates_page.test.tsx`.
// ─────────────────────────────────────────────────────────────────────────────
beforeAll(() => {
  class ResizeObserverStub {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  globalThis.ResizeObserver =
    ResizeObserverStub as unknown as typeof ResizeObserver;
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

// ─────────────────────────────────────────────────────────────────────────────
// El `ModeContextProvider` persiste el tema en `localStorage`, así que sin
// limpiarlo entre tests el estado de un test se cuela en el siguiente.
// ─────────────────────────────────────────────────────────────────────────────
beforeEach(() => localStorage.clear());
afterEach(() => localStorage.clear());

// ─────────────────────────────────────────────────────────────────────────────
// Contrato del delta `admin-sidebar-and-version`:
//   · requisito «El panel de administración no tiene barra superior» (sin
//     cambios: se conserva tal cual);
//   · requisito «El pie de la barra lateral gobierna la sesión, el tema y el
//     colapso» (sustituye al viejo «La sesión y el tema se gobiernan desde el
//     pie de la barra lateral»);
//   · requisito «La barra lateral lleva la marca de la aplicación»;
//   · requisito «La versión publicada se muestra y nunca aparece como
//     undefined».
// ─────────────────────────────────────────────────────────────────────────────

/** Etiquetas visibles, en inglés, de las tres entradas del pie. */
const THEME_LABEL = "Theme";
const LOGOUT_LABEL = "Log out";
const COLLAPSE_LABEL = "Collapse";
const FOOTER_LABELS = [THEME_LABEL, LOGOUT_LABEL, COLLAPSE_LABEL];

/** Entrada de navegación que identifica el menú de navegación. */
const NAV_LABEL = "Dashboard";

const SESSION: AuthContextInterface = {
  token: "test-token",
  isLoggedIn: true,
  login: () => {},
  logout: () => {},
};

const ANON: AuthContextInterface = {
  token: null,
  isLoggedIn: false,
  login: () => {},
  logout: () => {},
};

interface RenderOptions {
  /**
   * El `ModeContext` por defecto no conmuta nada, así que el test del tema
   * necesita el provider real para que la entrada cambie el modo persistido.
   */
  withModeProvider?: boolean;
}

function renderAdmin(
  auth: AuthContextInterface,
  options: RenderOptions = {},
): void {
  const tree = (
    <AuthContext.Provider value={auth}>
      <MemoryRouter initialEntries={["/admin"]}>
        <Routes>
          <Route path="/" element={<div>portada pública</div>} />
          <Route path="/login" element={<div>pantalla de login</div>} />
          <Route path="/admin" element={<AdminLayout />} />
          {/* Destino real de la entrada de cerrar sesión del pie. */}
          <Route path="/admin/logout" element={<div>sesión cerrada</div>} />
        </Routes>
      </MemoryRouter>
    </AuthContext.Provider>
  );

  render(
    options.withModeProvider ? (
      <ModeContextProvider>{tree}</ModeContextProvider>
    ) : (
      tree
    ),
  );
}

/** La barra lateral del panel. */
function getSider(): HTMLElement {
  const sider = document.querySelector<HTMLElement>(".ant-layout-sider");
  expect(
    sider,
    "el panel debe seguir pintando la barra lateral (ant-layout-sider)",
  ).not.toBeNull();
  if (!sider) {
    throw new Error("No hay barra lateral");
  }
  return sider;
}

/** El cuerpo de la barra lateral, donde viven la marca, la navegación y el pie. */
function getSiderBody(): HTMLElement {
  const body = getSider().querySelector<HTMLElement>(
    ".ant-layout-sider-children",
  );
  expect(
    body,
    "la barra lateral debe tener cuerpo (ant-layout-sider-children)",
  ).not.toBeNull();
  if (!body) {
    throw new Error("La barra lateral no tiene cuerpo");
  }
  return body;
}

/**
 * Una entrada del pie, localizada por su etiqueta visible.
 *
 * Se busca dentro de la barra lateral y por el texto de la entrada —no por su
 * nombre accesible—: los iconos de antd pintan `<span role="img" aria-label>`
 * dentro, y ese `aria-label` participa del nombre accesible, así que una entrada
 * sin etiqueta podría colarse por el nombre del icono.
 */
function getEntryByLabel(label: string): HTMLElement {
  const matches = within(getSider())
    .queryAllByRole("menuitem")
    .filter((entry) => (entry.textContent ?? "").includes(label));

  expect(
    matches,
    `la barra lateral debe contener una única entrada de menú rotulada «${label}»`,
  ).toHaveLength(1);
  if (matches.length !== 1) {
    throw new Error(`No hay una única entrada de menú rotulada «${label}»`);
  }
  return matches[0];
}

/**
 * El bloque del pie: el menú (`ul.ant-menu`) que contiene la entrada de cerrar
 * sesión.
 *
 * El `marginTop: "auto"` del contrato viejo ya no es el anclaje: el pie es un
 * `Menu` hermano del de navegación, y lo que lo identifica es ser el bloque que
 * aloja la entrada de cerrar sesión.
 */
function getSiderFooter(): HTMLElement {
  const footer = getEntryByLabel(LOGOUT_LABEL).closest<HTMLElement>(
    "ul.ant-menu",
  );
  if (!footer) {
    throw new Error(
      "el bloque del pie debe ser el menú (ul.ant-menu) que contiene la entrada de cerrar sesión",
    );
  }
  return footer;
}

/** El menú de navegación: el que contiene la entrada de «Dashboard». */
function getNavMenu(): HTMLElement {
  const nav = within(getSider())
    .getByText(NAV_LABEL)
    .closest<HTMLElement>("ul.ant-menu");
  expect(
    nav,
    "la barra lateral debe seguir teniendo su menú de navegación",
  ).not.toBeNull();
  if (!nav) {
    throw new Error("No hay menú de navegación");
  }
  return nav;
}

describe("AdminLayout — el panel no tiene barra superior", () => {
  // ─── Tarea 1.1 ───────────────────────────────────────────────────────────
  it("no pinta ninguna cabecera y conserva la barra lateral y el pie", () => {
    renderAdmin(SESSION);

    expect(document.querySelectorAll(".ant-layout-header")).toHaveLength(0);
    // La ausencia de la *clase* no basta: el contrato prohíbe el elemento
    // semántico, y una cabecera sin la clase de antd seguiría siendo cabecera.
    expect(document.querySelectorAll("header")).toHaveLength(0);
    expect(document.querySelectorAll(".ant-layout-sider")).toHaveLength(1);
    expect(document.querySelectorAll(".ant-layout-footer")).toHaveLength(1);
  });
});

describe("AdminLayout — el pie de la barra lateral gobierna la sesión, el tema y el colapso", () => {
  // ─── Tarea 1.3 ───────────────────────────────────────────────────────────
  it("sitúa las tres entradas al pie, con forma de menú y fuera de la navegación", () => {
    renderAdmin(SESSION);

    const footer = getSiderFooter();
    const theme = getEntryByLabel(THEME_LABEL);
    const logout = getEntryByLabel(LOGOUT_LABEL);
    const collapse = getEntryByLabel(COLLAPSE_LABEL);

    // Cada entrada lleva su etiqueta visible en inglés. Se exige el *texto*, que
    // es más fuerte que el nombre accesible: un icono de antd dentro de la
    // entrada aporta su propio `aria-label` y podría hacer pasar una entrada sin
    // etiqueta.
    expect(theme).toHaveTextContent(THEME_LABEL);
    expect(logout).toHaveTextContent(LOGOUT_LABEL);
    expect(collapse).toHaveTextContent(COLLAPSE_LABEL);

    // Las tres comparten el mismo bloque...
    expect(theme.closest("ul.ant-menu")).toBe(footer);
    expect(logout.closest("ul.ant-menu")).toBe(footer);
    expect(collapse.closest("ul.ant-menu")).toBe(footer);

    // ...que NO es el menú de navegación: el bloque del pie no puede contenerlo.
    const navMenu = getNavMenu();
    expect(footer).not.toBe(navMenu);
    expect(footer.contains(navMenu)).toBe(false);
    expect(navMenu.contains(footer)).toBe(false);

    // El pie cierra *lo visible* del cuerpo de la barra. No se exige que no
    // exista ningún nodo después: rc-menu pinta siempre detrás del `ul` de cada
    // menú un `div` oculto de medida (`aria-hidden="true"`, `display: none`) que
    // no se pinta, así que lo que el contrato prohíbe es un bloque *visible* por
    // debajo del pie, no la mera existencia de un nodo.
    for (
      let sibling = footer.nextElementSibling;
      sibling !== null;
      sibling = sibling.nextElementSibling
    ) {
      const style = window.getComputedStyle(sibling);
      const hidden =
        sibling.getAttribute("aria-hidden") === "true" ||
        style.display === "none" ||
        style.visibility === "hidden";
      expect(
        hidden,
        "después del pie no puede quedar ningún bloque visible",
      ).toBe(true);
    }
  });

  // ─── Tarea 1.4 ───────────────────────────────────────────────────────────
  it("navega al cierre de sesión al pulsar la entrada del pie", async () => {
    const user = userEvent.setup();
    renderAdmin(SESSION);

    await user.click(getEntryByLabel(LOGOUT_LABEL));

    expect(await screen.findByText("sesión cerrada")).toBeTruthy();
  });

  // ─── Tarea 1.5 ───────────────────────────────────────────────────────────
  it("pliega el bloque del pie a iconos cuando la barra está colapsada", async () => {
    const user = userEvent.setup();
    renderAdmin(SESSION);

    expect(getSiderFooter()).not.toHaveClass("ant-menu-inline-collapsed");

    await user.click(getEntryByLabel(COLLAPSE_LABEL));

    await waitFor(() =>
      expect(getSiderFooter()).toHaveClass("ant-menu-inline-collapsed"),
    );

    // Las tres entradas siguen ahí, con sus etiquetas: el plegado es visual.
    for (const label of FOOTER_LABELS) {
      expect(getEntryByLabel(label)).toHaveTextContent(label);
    }
  });

  // ─── Tarea 1.6 ───────────────────────────────────────────────────────────
  it("anuncia el estado de la barra y alterna al pulsar la entrada de colapso", async () => {
    const user = userEvent.setup();
    renderAdmin(SESSION);

    const sider = getSider();
    const collapseEntry = (): HTMLElement => getEntryByLabel(COLLAPSE_LABEL);
    const collapsed = (): boolean =>
      sider.classList.contains("ant-layout-sider-collapsed");

    expect(collapsed()).toBe(false);
    // La entrada anuncia su estado, no sólo el icono que lleva dentro.
    expect(collapseEntry().getAttribute("aria-expanded")).toBe("true");

    // Y declara qué controla: el identificador de la barra lateral.
    const controls = collapseEntry().getAttribute("aria-controls");
    expect(
      controls,
      "la entrada de colapso debe declarar con aria-controls qué barra controla",
    ).toBeTruthy();
    expect(sider.id).not.toBe("");
    expect(controls).toBe(sider.id);

    await user.click(collapseEntry());
    await waitFor(() => expect(collapsed()).toBe(true));
    expect(collapseEntry().getAttribute("aria-expanded")).toBe("false");

    await user.click(collapseEntry());
    await waitFor(() => expect(collapsed()).toBe(false));
    expect(collapseEntry().getAttribute("aria-expanded")).toBe("true");
  });

  // ─── Tarea 1.7 ───────────────────────────────────────────────────────────
  it("es única y localiza todas las entradas del panel dentro de la barra lateral", () => {
    renderAdmin(SESSION);

    const sider = getSider();

    // Un duplicado (una entrada de cerrar sesión en la barra y otra fuera)
    // rompería el contrato de unicidad.
    const logoutEntries = screen
      .queryAllByRole("menuitem")
      .filter((entry) => (entry.textContent ?? "").includes(LOGOUT_LABEL));
    expect(logoutEntries).toHaveLength(1);

    // Y ninguna entrada de menú del panel vive fuera de la barra lateral.
    const allEntries = screen.getAllByRole("menuitem");
    expect(allEntries.length).toBeGreaterThan(0);
    for (const entry of allEntries) {
      expect(
        sider.contains(entry),
        "toda entrada de menú del panel debe vivir dentro de la barra lateral",
      ).toBe(true);
    }
  });

  // ─── Tarea 1.8 ───────────────────────────────────────────────────────────
  it("cambia el tema al pulsar su entrada y refleja el tema activo en el icono", async () => {
    const user = userEvent.setup();
    renderAdmin(SESSION, { withModeProvider: true });

    const themeEntry = (): HTMLElement => getEntryByLabel(THEME_LABEL);

    // Sin nada persistido el provider arranca en oscuro: luna, y nada escrito.
    expect(themeEntry().querySelector(".anticon-moon")).not.toBeNull();
    expect(localStorage.getItem("mode")).toBeNull();

    await user.click(themeEntry());

    await waitFor(() => expect(localStorage.getItem("mode")).toBe("light"));
    expect(themeEntry().querySelector(".anticon-sun")).not.toBeNull();
  });

  // ─── Tarea 1.9 ───────────────────────────────────────────────────────────
  it("pega la barra a la ventana y deja que el menú de navegación se desplace dentro", () => {
    renderAdmin(SESSION);

    // jsdom no calcula layout, así que aquí se fija el *mecanismo* declarado y
    // no el resultado pintado, igual que hacen los tests de la barra de
    // herramientas de plantillas. El mecanismo es el que hace que el pie siga a
    // la vista en las páginas más altas que la ventana: la barra se pega a la
    // ventana y ocupa su altura, su cuerpo es una columna flex, y es el menú de
    // navegación —no el pie— el que se desplaza cuando no cabe.
    const sider = getSider();
    expect(sider.style.position).toBe("sticky");
    expect(sider.style.top).toBe("0px");
    expect(sider.style.height).toBe("100vh");

    const body = getSiderBody();
    expect(body.style.display).toBe("flex");
    expect(body.style.flexDirection).toBe("column");

    const navMenu = getNavMenu();
    expect(navMenu.style.overflowY).toBe("auto");
    expect(navMenu.style.minHeight).toBe("0px");
  });

  // ─── Tarea 1.10 ──────────────────────────────────────────────────────────
  it("encabeza la barra con la marca y la pliega a solo el icono", async () => {
    const user = userEvent.setup();
    renderAdmin(SESSION);

    const brand = getSiderBody().firstElementChild as HTMLElement | null;
    expect(
      brand,
      "la marca debe encabezar el cuerpo de la barra lateral",
    ).not.toBeNull();
    if (brand === null) {
      return;
    }

    const logo = brand.querySelector("img");
    expect(
      logo,
      "la marca debe llevar el icono de la aplicación",
    ).not.toBeNull();
    expect(logo?.getAttribute("src")).toMatch(/logo/i);
    expect(brand).toHaveTextContent("Shuul");

    // La navegación viene después de la marca.
    const navMenu = getNavMenu();
    expect(
      brand.compareDocumentPosition(navMenu) & Node.DOCUMENT_POSITION_FOLLOWING,
      "el menú de navegación debe venir después de la marca",
    ).not.toBe(0);

    // Con la barra colapsada se queda el icono y desaparece el nombre.
    await user.click(getEntryByLabel(COLLAPSE_LABEL));
    await waitFor(() =>
      expect(getSider().classList.contains("ant-layout-sider-collapsed")).toBe(
        true,
      ),
    );

    const collapsedBrand = getSiderBody()
      .firstElementChild as HTMLElement | null;
    expect(collapsedBrand).not.toBeNull();
    expect(collapsedBrand?.querySelector("img")).not.toBeNull();
    expect(collapsedBrand).not.toHaveTextContent("Shuul");
  });

  // ─── Tarea 1.11 ──────────────────────────────────────────────────────────
  it("el pie de página no imprime undefined", () => {
    renderAdmin(SESSION);

    const footer = document.querySelector<HTMLElement>(".ant-layout-footer");
    expect(footer).not.toBeNull();
    if (footer === null) {
      return;
    }

    expect(footer).toHaveTextContent("Shuul");
    expect(footer).not.toHaveTextContent("undefined");
  });

  // ─── Tarea 1.6 (contrato viejo, sin cambios) ─────────────────────────────
  it("sin sesión devuelve a la portada pública, no a una pantalla de login", async () => {
    renderAdmin(ANON);

    expect(await screen.findByText("portada pública")).toBeTruthy();
    expect(screen.queryByText("pantalla de login")).toBeNull();
  });
});
