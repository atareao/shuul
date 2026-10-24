import { describe, it, expect } from "vitest";
import { describeRuleImport } from "./rule";
import type { RejectedRule, RuleImportResult } from "./rule";

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
