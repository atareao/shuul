import { describe, it, expect } from "vitest";
import { describeRuleImport } from "./rule";

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
});
