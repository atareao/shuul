import { describe, it, expect, vi, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";

import App from "./App";

// ─────────────────────────────────────────────────────────────────────────────
// Setup de entorno de navegador mínimo para antd (jsdom carece de estas APIs).
// El montaje completo arrastra además i18n con su backend HTTP, que en jsdom no
// resuelve: el ruido en consola es esperado y no afecta a lo que se afirma.
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
    // No se queda en blanco.
    expect((document.body.textContent ?? "").trim().length).toBeGreaterThan(0);
  }, 30000);
});
