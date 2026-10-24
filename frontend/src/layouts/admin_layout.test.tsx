import { describe, it, expect, vi, beforeAll } from "vitest";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Routes, Route } from "react-router";

import AdminLayout from "./admin_layout";
import AuthContext from "@/components/auth_context";
import type { AuthContextInterface } from "@/components/auth_context";

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
// Contrato del delta `ui-shell-and-toolbar`, requisito «El panel de
// administración no tiene barra superior» y requisito «La sesión y el tema se
// gobiernan desde el pie de la barra lateral».
// ─────────────────────────────────────────────────────────────────────────────

/** Nombre accesible del botón de cerrar sesión (es un botón de solo icono). */
const LOGOUT_NAME = /log\s*out|sign\s*out/i;
/** Nombre accesible del control de colapsar (cambia con el estado de la barra). */
const COLLAPSE_NAME = /collapse|expand|toggle\s*sidebar/i;
/** Nombre accesible del conmutador de tema, que ya existe y no debe perderse. */
const THEME_NAME = /toggle dark mode/i;

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

function renderAdmin(auth: AuthContextInterface): void {
  render(
    <AuthContext.Provider value={auth}>
      <MemoryRouter initialEntries={["/admin"]}>
        <Routes>
          <Route path="/" element={<div>portada pública</div>} />
          <Route path="/login" element={<div>pantalla de login</div>} />
          <Route path="/admin" element={<AdminLayout />} />
          {/* Destino real del botón de cerrar sesión del pie. */}
          <Route path="/admin/logout" element={<div>sesión cerrada</div>} />
        </Routes>
      </MemoryRouter>
    </AuthContext.Provider>,
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

/**
 * El contenedor que comparten los tres controles: el ancestro más cercano del
 * botón de cerrar sesión que declara su anclaje abajo (`margin-top: auto`).
 *
 * Se localiza subiendo desde el botón de cerrar sesión —que tiene que estar
 * dentro de la barra lateral— y no buscando un `div` por su clase: si el
 * contenedor que compartieran los tres fuera la propia barra lateral, el
 * anclaje y la exclusión del menú que se afirman más abajo no probarían nada.
 */
function getSiderFooter(): HTMLElement {
  const sider = getSider();
  const logout = within(sider).getByRole("button", { name: LOGOUT_NAME });
  let node: HTMLElement | null = logout;
  while (node !== null && node !== sider) {
    if (node.style.marginTop === "auto") {
      return node;
    }
    node = node.parentElement;
  }
  throw new Error(
    'El pie de la barra lateral debe declarar su anclaje abajo con marginTop: "auto"',
  );
}

/**
 * Exige que el control declare su propio nombre accesible con `aria-label`.
 *
 * El requisito es explícito: el nombre tiene que ser del control, no del icono
 * que lleve dentro. Un `getByRole(…, { name })` puede acertar aunque el
 * atributo no exista —antd pinta `<span role="img" aria-label="logout">` dentro
 * del botón—, así que aquí se comprueba el atributo en el propio control.
 */
function expectOwnAriaLabel(control: HTMLElement, what: string): void {
  const label = control.getAttribute("aria-label");
  expect(
    typeof label === "string" && label.trim().length > 0,
    `${what} debe declarar su propio aria-label, no vacío, sin depender del que aporte el icono de dentro`,
  ).toBe(true);
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

describe("AdminLayout — la sesión y el tema se gobiernan desde el pie de la barra lateral", () => {
  // ─── Tarea 1.2 ───────────────────────────────────────────────────────────
  it("sitúa cerrar sesión, el tema y el colapsar en un mismo contenedor anclado abajo, fuera del menú", async () => {
    const user = userEvent.setup();
    renderAdmin(SESSION);

    const sider = getSider();
    const logout = within(sider).queryByRole("button", { name: LOGOUT_NAME });
    expect(
      logout,
      "el botón de cerrar sesión debe vivir dentro de la barra lateral",
    ).not.toBeNull();
    if (!logout) {
      return;
    }

    const footer = getSiderFooter();

    // El pie es un contenedor propio dentro de la barra, no la barra entera:
    // el menú de navegación vive en la barra pero no en el pie.
    expect(footer, "el pie no puede ser la propia barra lateral").not.toBe(
      sider,
    );
    expect(sider.contains(footer)).toBe(true);
    expect(footer.querySelector(".ant-menu")).toBeNull();
    expect(sider.querySelector(".ant-menu")).not.toBeNull();

    // Los tres controles comparten ese contenedor.
    expect(footer.contains(logout)).toBe(true);
    expect(
      within(footer).getByRole("switch", { name: THEME_NAME }),
    ).toBeTruthy();
    expect(
      within(footer).getByRole("button", { name: COLLAPSE_NAME }),
    ).toBeTruthy();

    // El pie cierra su contenedor: no queda nada por debajo de él. No basta
    // con que declare `marginTop: "auto"` para *pedir* el fondo.
    expect(footer.nextElementSibling).toBeNull();

    // El control de cerrar sesión no es decorativo: al pulsarlo se navega al
    // cierre de sesión. Es cobertura que faltaba, no comportamiento nuevo.
    await user.click(logout);
    expect(await screen.findByText("sesión cerrada")).toBeTruthy();
  });

  // ─── Tarea 1.3 ───────────────────────────────────────────────────────────
  it("coloca los tres controles en fila con la barra desplegada y en columna al colapsarla", async () => {
    const user = userEvent.setup();
    renderAdmin(SESSION);

    expect(getSiderFooter().style.flexDirection).toBe("row");

    await user.click(
      within(getSider()).getByRole("button", { name: COLLAPSE_NAME }),
    );

    await waitFor(() =>
      expect(getSiderFooter().style.flexDirection).toBe("column"),
    );
  });

  // ─── Tarea 1.4 ───────────────────────────────────────────────────────────
  it("da nombre accesible a cerrar sesión y a colapsar, y conserva el del tema", () => {
    renderAdmin(SESSION);

    const sider = getSider();

    // El nombre accesible tiene que ser del control, no del icono de dentro:
    // los dos botones de solo icono declaran su propio `aria-label`.
    expectOwnAriaLabel(
      screen.getByRole("button", { name: LOGOUT_NAME }),
      "el botón de cerrar sesión",
    );
    expectOwnAriaLabel(
      screen.getByRole("button", { name: COLLAPSE_NAME }),
      "el control de colapsar",
    );

    expect(
      within(sider).getByRole("button", { name: LOGOUT_NAME }),
    ).toBeTruthy();
    expect(
      within(sider).getByRole("button", { name: COLLAPSE_NAME }),
    ).toBeTruthy();
    expect(
      within(sider).getByRole("switch", { name: THEME_NAME }),
    ).toBeTruthy();

    // El control de cerrar sesión es único en todo el documento: un duplicado
    // (uno en la barra y otro fuera) rompería el contrato de localización.
    expect(screen.getAllByRole("button", { name: LOGOUT_NAME })).toHaveLength(1);

    // Y ningún botón del panel vive fuera de la barra lateral.
    const layoutButtons = screen.getAllByRole("button");
    expect(layoutButtons.length).toBeGreaterThan(0);
    for (const button of layoutButtons) {
      expect(
        sider.contains(button),
        "todo botón del panel debe vivir dentro de la barra lateral",
      ).toBe(true);
    }
  });

  // ─── Tarea 1.5 ───────────────────────────────────────────────────────────
  it("alterna la clase de colapsada de la barra en cada pulsación del control", async () => {
    const user = userEvent.setup();
    renderAdmin(SESSION);

    const collapseButton = (): HTMLElement =>
      within(getSider()).getByRole("button", { name: COLLAPSE_NAME });
    const collapsed = (): boolean =>
      getSider().classList.contains("ant-layout-sider-collapsed");

    expect(collapsed()).toBe(false);
    // El control anuncia su estado, no sólo el icono que lleva dentro.
    expect(collapseButton().getAttribute("aria-expanded")).toBe("true");

    await user.click(collapseButton());
    await waitFor(() => expect(collapsed()).toBe(true));
    expect(collapseButton().getAttribute("aria-expanded")).toBe("false");

    await user.click(collapseButton());
    await waitFor(() => expect(collapsed()).toBe(false));
    expect(collapseButton().getAttribute("aria-expanded")).toBe("true");
  });

  // ─── Tarea 1.6 ───────────────────────────────────────────────────────────
  it("sin sesión devuelve a la portada pública, no a una pantalla de login", async () => {
    renderAdmin(ANON);

    expect(await screen.findByText("portada pública")).toBeTruthy();
    expect(screen.queryByText("pantalla de login")).toBeNull();
  });
});
