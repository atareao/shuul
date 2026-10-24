import { describe, it, expect, vi, beforeAll, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";

import HomePage from "./home_page";
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

// ─────────────────────────────────────────────────────────────────────────────
// Contrato del delta `ui-shell-and-toolbar`, requisito «La portada pública es la
// entrada al panel»: la portada ofrece un botón rotulado que arranca el SSO.
// ─────────────────────────────────────────────────────────────────────────────
describe("HomePage — la portada es la entrada al panel", () => {
  it("ofrece un botón rotulado que arranca el SSO al pulsarlo", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <HomePage />
      </MemoryRouter>,
    );

    const button = screen.getByRole("button", { name: /sign in|log\s*in/i });
    await user.click(button);

    expect(startSsoLogin).toHaveBeenCalledTimes(1);
  });
});
