import { describe, it, expect, vi, beforeAll, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import App from "./App";

// ─────────────────────────────────────────────────────────────────────────────
// El backend HTTP de i18n no resuelve en jsdom: dejaba XHR fallidos en consola.
// Se sustituye por un backend en memoria que resuelve vacío al instante. El
// texto que afirma este test es literal en la portada, así que no depende de
// ninguna traducción.
// ─────────────────────────────────────────────────────────────────────────────
vi.mock("i18next-http-backend", () => ({
  default: class MockBackend {
    // i18next identifica el módulo por este `type` estático.
    static type = "backend";

    read(
      _lng: string,
      _ns: string,
      callback: (err: unknown, data: Record<string, unknown>) => void,
    ): void {
      callback(null, {});
    }
  },
}));

// ─────────────────────────────────────────────────────────────────────────────
// Setup de entorno de navegador mínimo para antd (jsdom carece de estas APIs).
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
// Contrato del delta `ui-shell-and-toolbar`, requisito «La sesión cerrada
// devuelve a la portada pública»: `/login` ya no es una ruta propia y visitarla
// acaba en la portada, sin dejar la pantalla en blanco.
//
// El discriminador es el título de la portada: el rótulo del botón de iniciar
// sesión lo comparten la portada y la página de login eliminada, así que no
// sirve para distinguirlas.
// ─────────────────────────────────────────────────────────────────────────────
describe("App — /login ya no es una ruta propia", () => {
  // El test muta la URL con `pushState`; se devuelve a su sitio para no dejar
  // la mutación al resto del fichero.
  afterEach(() => {
    window.history.pushState({}, "", "/");
  });

  it("una visita directa a /login acaba en la portada pública", async () => {
    window.history.pushState({}, "", "/login");

    render(<App />);

    expect(
      await screen.findByText(
        "The gatekeeper of your data",
        {},
        { timeout: 15000 },
      ),
    ).toBeTruthy();

    // No basta con que se pinte la portada: la visita tiene que *acabar* en
    // `/`. Una ruta `/login` que renderizara la portada sin redirigir pasaría
    // el resto del test y violaría el escenario.
    await waitFor(() => expect(window.location.pathname).toBe("/"));

    // Ni pantalla en blanco ni el respaldo del `Suspense`.
    expect(screen.queryByText("Loading...")).toBeNull();
  }, 30000);
});
