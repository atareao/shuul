import { describe, it, expect } from "vitest";

import { TITLE, formatTitle } from "./constants";

// ─────────────────────────────────────────────────────────────────────────────
// Contrato del delta `admin-sidebar-and-version`, requisito «La versión
// publicada se muestra y nunca aparece como undefined».
//
// El título de la aplicación lo consumen el título del Dashboard y el pie del
// panel. Cuando la versión no está disponible —el build de producción no la
// declaró, la variable llegó vacía o traía solo espacios— el título NO puede
// imprimir la cadena `undefined`: tiene que quedarse en el nombre a secas.
// ─────────────────────────────────────────────────────────────────────────────

/** Mensaje común: en RED `formatTitle` todavía no existe en `constants`. */
const MISSING = "`formatTitle` debe existir en `constants`";

describe("constants — el título de la aplicación", () => {
  it("compone el nombre con la versión entre paréntesis", () => {
    expect(typeof formatTitle, MISSING).toBe("function");
    if (typeof formatTitle !== "function") {
      return;
    }

    expect(formatTitle("0.20.0")).toBe("Shuul (0.20.0)");
  });

  it("omite el paréntesis cuando no hay versión", () => {
    expect(typeof formatTitle, MISSING).toBe("function");
    if (typeof formatTitle !== "function") {
      return;
    }

    // Sin versión —ausente, vacía o solo espacios— no hay paréntesis que abrir.
    expect(formatTitle(undefined)).toBe("Shuul");
    expect(formatTitle("")).toBe("Shuul");
    expect(formatTitle("   ")).toBe("Shuul");

    expect(formatTitle(undefined)).not.toContain("undefined");
    expect(formatTitle("")).not.toContain("undefined");
    expect(formatTitle("   ")).not.toContain("undefined");
  });

  it("`TITLE` nunca contiene la cadena undefined", () => {
    // La aserción de tipo va primero a propósito: en RED el fallo tiene que ser
    // un mensaje claro, no un `TypeError` al usar un valor inexistente.
    expect(
      typeof TITLE,
      "`TITLE` debe existir en `constants` (lo consumen el título del Dashboard y el pie del panel)",
    ).toBe("string");
    if (typeof TITLE !== "string") {
      return;
    }

    expect(TITLE).not.toContain("undefined");
    expect(TITLE).toContain("Shuul");
  });
});
