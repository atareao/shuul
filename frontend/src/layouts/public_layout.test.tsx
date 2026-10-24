import { describe, it, expect, vi, beforeAll, beforeEach } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Routes, Route } from "react-router";

import PublicLayout from "./public_layout";
import { startSsoLogin } from "@/common/auth";

// El helper del SSO se sustituye por un mock: la navegación de verdad es cosa
// suya, así que el test no depende de que jsdom navegue.
vi.mock("@/common/auth", () => ({
  startSsoLogin: vi.fn(),
}));

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

beforeEach(() => {
  vi.mocked(startSsoLogin).mockClear();
});

function renderLayout(): void {
  render(
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route path="/" element={<PublicLayout />}>
          <Route index element={<div>contenido público</div>} />
        </Route>
        <Route path="/login" element={<div>pantalla de login</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Contrato del delta `ui-shell-and-toolbar`, requisito «La portada pública es la
// entrada al panel»: el icono de la cabecera pública arranca el SSO sin pasar
// por ninguna ruta intermedia.
// ─────────────────────────────────────────────────────────────────────────────
describe("PublicLayout — el icono de la cabecera arranca el SSO", () => {
  it("arranca el SSO desde su control de la cabecera, sin navegar a /login", async () => {
    const user = userEvent.setup();
    renderLayout();

    const header = document.querySelector<HTMLElement>(".ant-layout-header");
    expect(
      header,
      "la parte pública conserva su cabecera (ant-layout-header)",
    ).not.toBeNull();
    if (!header) {
      return;
    }

    const button = within(header).getByRole("button", {
      name: /sign in|log\s*in/i,
    });
    await user.click(button);

    expect(startSsoLogin).toHaveBeenCalledTimes(1);
    // No pasa por la ruta intermedia: el marcador de `/login` no aparece.
    expect(screen.queryByText("pantalla de login")).toBeNull();
  });
});
