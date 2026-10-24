import { describe, it, expect, vi, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router";

import PublicLayout from "./public_layout";

// antd necesita matchMedia en jsdom.
beforeAll(() => {
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

function renderLayout(): void {
  render(
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route path="/" element={<PublicLayout />}>
          <Route index element={<div>contenido público</div>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Contrato del delta `public-landing-without-header`, requisito «La parte pública
// no pinta cabecera»: la envoltura pública no pinta cabecera superior ni ofrece
// un control de inicio de sesión propio, pero conserva su pie de página y el
// contenido de la ruta hija.
// ─────────────────────────────────────────────────────────────────────────────
describe("PublicLayout — la parte pública no pinta cabecera", () => {
  it("no pinta ninguna cabecera superior", () => {
    renderLayout();

    expect(document.querySelector(".ant-layout-header")).toBeNull();
  });

  it("no ofrece ningún control de inicio de sesión en la envoltura", () => {
    renderLayout();

    expect(
      screen.queryByRole("button", { name: /sign in|log\s*in/i }),
    ).toBeNull();
  });

  it("conserva el pie de página con el copyright", () => {
    renderLayout();

    const footer = document.querySelector<HTMLElement>(".ant-layout-footer");
    expect(
      footer,
      "la parte pública debería conservar su pie de página (ant-layout-footer)",
    ).not.toBeNull();
    if (!footer) {
      return;
    }

    expect(footer).toHaveTextContent(`©${new Date().getFullYear()} atareao`);
  });

  it("sigue renderizando el contenido de la ruta hija", () => {
    renderLayout();

    expect(screen.getByText("contenido público")).toBeInTheDocument();
  });
});
