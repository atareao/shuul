import { describe, it, expect } from "vitest";
import {
  describeRuleImport,
  collectionRequiresKey,
  projectConditions,
  FIELD_COLLECTIONS,
  CONDITION_COLLECTIONS,
} from "./rule";
import type Rule from "./rule";
import type {
  Collection,
  Condition,
  ProjectableFields,
  RejectedRule,
  RuleImportResult,
} from "./rule";

describe("describeRuleImport", () => {
  it("con sólo importadas devuelve un éxito con el recuento", () => {
    const notice = describeRuleImport({ imported: 3, rejected: [] });

    expect(notice.kind).toBe("success");
    expect(notice.text).toBe("Imported 3 rules");
  });

  it("con rejected ausente trata la importación como limpia", () => {
    const notice = describeRuleImport({ imported: 2 });

    expect(notice.kind).toBe("success");
    expect(notice.text).toBe("Imported 2 rules");
  });

  it("con una rechazada avisa y la nombra con su motivo", () => {
    const notice = describeRuleImport({
      imported: 3,
      rejected: [{ name: "rogue-rule", reason: "unknown pipeline 'bogus'" }],
    });

    expect(notice.kind).toBe("warning");
    expect(notice.text).toContain("Imported 3 rules");
    expect(notice.text).toContain("1 rejected");
    expect(notice.text).toContain("rogue-rule");
    expect(notice.text).toContain("unknown pipeline 'bogus'");
  });

  it("nombra la rechazada aunque no venga el motivo", () => {
    const notice = describeRuleImport({
      imported: 1,
      rejected: [{ name: "nameless-reason" }],
    });

    expect(notice.kind).toBe("warning");
    expect(notice.text).toContain("nameless-reason");
    expect(notice.text).not.toContain("undefined");
  });

  it("enumera varias rechazadas", () => {
    const notice = describeRuleImport({
      imported: 0,
      rejected: [{ name: "first" }, { name: "second", reason: "bad" }],
    });

    expect(notice.kind).toBe("warning");
    expect(notice.text).toContain("2 rejected");
    expect(notice.text).toContain("first");
    expect(notice.text).toContain("second (bad)");
  });

  it("acota la enumeración a tres rechazadas y resume las restantes", () => {
    const notice = describeRuleImport({
      imported: 2,
      rejected: [
        { name: "first" },
        { name: "second" },
        { name: "third" },
        { name: "fourth" },
        { name: "fifth" },
      ],
    });

    expect(notice.kind).toBe("warning");
    expect(notice.text).toBe(
      "Imported 2 rules, 5 rejected: first, second, third, … and 2 more",
    );
    expect(notice.text).not.toContain("fourth");
    expect(notice.text).not.toContain("fifth");
  });

  it("con tres rechazadas justas las enumera todas y no resume nada", () => {
    const notice = describeRuleImport({
      imported: 0,
      rejected: [{ name: "a" }, { name: "b" }, { name: "c" }],
    });

    expect(notice.kind).toBe("warning");
    expect(notice.text).toBe("Imported 0 rules, 3 rejected: a, b, c");
    expect(notice.text).not.toContain("more");
  });

  it("con `imported` ausente no imprime undefined", () => {
    const notice = describeRuleImport({
      rejected: [],
    } as unknown as RuleImportResult);

    expect(notice.kind).toBe("success");
    expect(notice.text).toBe("Imported 0 rules");
    expect(notice.text).not.toContain("undefined");
  });

  it("con `imported` no finito no imprime NaN ni Infinity", () => {
    const notANumber = describeRuleImport({
      imported: Number.NaN,
      rejected: [],
    });
    const infinite = describeRuleImport({
      imported: Number.POSITIVE_INFINITY,
      rejected: [],
    });

    expect(notANumber.kind).toBe("success");
    expect(notANumber.text).toBe("Imported 0 rules");
    expect(infinite.text).toBe("Imported 0 rules");
    expect(notANumber.text).not.toContain("NaN");
    expect(infinite.text).not.toContain("Infinity");
  });

  it("con una rechazada sin nombre utilizable no imprime undefined", () => {
    const withoutName = describeRuleImport({
      imported: 0,
      rejected: [{ reason: "bad" } as unknown as RejectedRule],
    });
    const wrongType = describeRuleImport({
      imported: 0,
      rejected: [{ name: 42, reason: "bad" } as unknown as RejectedRule],
    });
    const blankName = describeRuleImport({
      imported: 0,
      rejected: [{ name: "   " }],
    });

    expect(withoutName.text).toBe(
      "Imported 0 rules, 1 rejected: unnamed rule (bad)",
    );
    expect(wrongType.text).toBe(
      "Imported 0 rules, 1 rejected: unnamed rule (bad)",
    );
    expect(blankName.text).toBe("Imported 0 rules, 1 rejected: unnamed rule");
    expect(withoutName.text).not.toContain("undefined");
    expect(wrongType.text).not.toContain("undefined");
    expect(blankName.text).not.toContain("undefined");
  });

  it("el resumen del resto convive con una entrada sin nombre", () => {
    const notice = describeRuleImport({
      imported: 1,
      rejected: [
        { name: "first" },
        { reason: "bad" } as unknown as RejectedRule,
        { name: "third" },
        { name: "fourth" },
      ],
    });

    expect(notice.text).toBe(
      "Imported 1 rules, 4 rejected: first, unnamed rule (bad), third, … and 1 more",
    );
    expect(notice.text).not.toContain("undefined");
  });
});

// ─── Condiciones: los catálogos, la clave por colección y la proyección ───────
// El backend ya persiste y evalúa un conjunto ordenado de condiciones (F2a,
// F2b-1, F2b-2a). Para que el diálogo pueda autorizarlas necesita dos piezas del
// modelo: saber qué colecciones exigen clave —la cookie, la cabecera o el
// argumento del query— y proyectar los 14 campos planos **con el mismo criterio
// que la API**, de modo que cambiar de modo no pierda ninguna restricción.
//
// La proyección es idéntica a la del backend: recorre las 14 colecciones de
// campo en su orden canónico —que es el orden de evaluación— y emite una
// condición por cada campo con patrón no vacío (tras `trim`). `is_tor` nunca
// proyecta: no es una de las 14 colecciones.

/** Las 14 colecciones de campo, en el orden canónico de evaluación. */
const FIELD_COLLECTION_NAMES = [
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

/** Las cinco colecciones de F2b-2a: tres con clave y dos sin ella. */
const EXTRA_COLLECTIONS = [
  "request_cookies",
  "request_headers",
  "args_get",
  "request_basename",
  "request_extension",
] as const;

describe("collectionRequiresKey", () => {
  it("es verdadero exactamente para las tres colecciones con clave", () => {
    const keyed: readonly Collection[] = [
      "request_cookies",
      "request_headers",
      "args_get",
    ];
    const withoutKey: readonly Collection[] = [
      ...FIELD_COLLECTION_NAMES,
      "request_basename",
      "request_extension",
    ];

    for (const collection of keyed) {
      expect(collectionRequiresKey(collection)).toBe(true);
    }
    for (const collection of withoutKey) {
      expect(collectionRequiresKey(collection)).toBe(false);
    }
  });
});

describe("los catálogos de colecciones", () => {
  it("FIELD_COLLECTIONS son las 14 de campo, en orden canónico", () => {
    expect([...FIELD_COLLECTIONS]).toEqual([...FIELD_COLLECTION_NAMES]);
  });

  it("CONDITION_COLLECTIONS son las 19, sin repetidas", () => {
    expect(CONDITION_COLLECTIONS).toHaveLength(19);
    expect(new Set(CONDITION_COLLECTIONS).size).toBe(19);
    for (const collection of [
      ...FIELD_COLLECTION_NAMES,
      ...EXTRA_COLLECTIONS,
    ] as const) {
      expect(CONDITION_COLLECTIONS).toContain(collection);
    }
  });
});

describe("projectConditions", () => {
  it("proyecta los campos rellenos en orden canónico, no en orden de escritura", () => {
    // Escrito a la inversa del canónico a propósito: `user_agent` va después de
    // `path` en la lista de campos, así que la proyección no puede limitarse a
    // recorrer las claves del objeto.
    const rule: ProjectableFields = {
      user_agent: "curl/8.0",
      path: "/admin",
      transformations: "lowercase",
    };

    const conditions: Condition[] = projectConditions(rule);

    expect(conditions).toEqual([
      {
        collection: "path",
        key: null,
        transformations: "lowercase",
        operator: "@rx",
        pattern: "/admin",
      },
      {
        collection: "user_agent",
        key: null,
        transformations: "lowercase",
        operator: "@rx",
        pattern: "curl/8.0",
      },
    ]);
  });

  it("no proyecta `is_tor`, que no es una colección de campo", () => {
    const rule: Rule = {
      id: 1,
      name: "tor-only",
      description: "",
      mode: "enforce",
      transformations: "",
      is_tor: true,
    };

    expect(projectConditions(rule)).toEqual([]);
  });

  it("no proyecta los patrones vacíos ni los de solo espacios", () => {
    expect(
      projectConditions({ path: "", query: "   ", transformations: "lowercase" }),
    ).toEqual([]);
  });

  it("una regla sin restricciones proyecta el conjunto vacío", () => {
    const rule: Rule = {
      id: 2,
      name: "no-restrictions",
      description: "",
      mode: "off",
    };

    expect(projectConditions(rule)).toEqual([]);
  });

  it("usa la cadena vacía cuando la regla no declara transformaciones", () => {
    const rule: ProjectableFields = { path: "/admin" };

    expect(projectConditions(rule)).toEqual([
      {
        collection: "path",
        key: null,
        transformations: "",
        operator: "@rx",
        pattern: "/admin",
      },
    ]);
  });
});
