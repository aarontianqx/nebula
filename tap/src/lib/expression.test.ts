import { describe, expect, it } from "vitest";

import { checkTemplate, checkValueText, hasTemplate, parseValueText, valueToText, withReference } from "./expression";

describe("value text <-> DslValue", () => {
  it("stores a numeric literal in a number field as a number, anything else as typed", () => {
    expect(parseValueText("100", "number")).toBe(100);
    expect(parseValueText(" -5 ", "number")).toBe(-5);
    expect(parseValueText("1.5", "number")).toBe(1.5);
    expect(parseValueText("{{ i * 10 }}", "number")).toBe("{{ i * 10 }}");
    // States on the way to a number stay what was typed, they are not coerced to 0.
    expect(parseValueText("", "number")).toBe("");
    expect(parseValueText("-", "number")).toBe("-");
    expect(parseValueText("1.", "number")).toBe("1.");
    expect(parseValueText("true", "number")).toBe("true");
  });

  it("keeps typed text as text, even when it looks like a number", () => {
    expect(parseValueText("42", "text")).toBe("42");
    expect(parseValueText("true", "text")).toBe("true");
    expect(parseValueText("hello {{ name }}", "text")).toBe("hello {{ name }}");
  });

  it("types call arguments: numbers and booleans are literals, the rest text", () => {
    expect(parseValueText("7", "auto")).toBe(7);
    expect(parseValueText("true", "auto")).toBe(true);
    expect(parseValueText("false", "auto")).toBe(false);
    expect(parseValueText("True", "auto")).toBe("True");
    expect(parseValueText("{{ n + 1 }}", "auto")).toBe("{{ n + 1 }}");
    expect(parseValueText("", "auto")).toBe("");
  });

  it("round-trips what it shows", () => {
    for (const value of [0, -3, 2.5, "x", "{{ a }}"] as const) {
      expect(parseValueText(valueToText(value), typeof value === "number" ? "number" : "auto")).toBe(value);
    }
    expect(valueToText(true)).toBe("true");
  });

  it("knows a template from a literal", () => {
    expect(hasTemplate("{{ x }}")).toBe(true);
    expect(hasTemplate("a {{ x }} b")).toBe(true);
    expect(hasTemplate("{ x }")).toBe(false);
  });
});

describe("template lint", () => {
  it("accepts plain text and well-formed templates", () => {
    for (const ok of [
      "",
      "plain",
      "{{ x }}",
      "{{x}}",
      "{{ x + (y * 2) }}",
      "{{ a }} and {{ b }}",
      'hello {{ "wor" + "ld" }}',
      "{{ [1, 2, 3][0] }}",
      '{{ "a ) b" }}',
      "{{ 'it\\'s' }}",
    ]) {
      expect(checkTemplate(ok), ok).toBeNull();
    }
  });

  it("finds a template that is never closed or has nothing in it", () => {
    expect(checkTemplate("{{ x")).toMatch(/closing/);
    expect(checkTemplate("a {{ x } b")).toMatch(/closing/);
    expect(checkTemplate("{{}}")).toMatch(/Empty/);
    expect(checkTemplate("{{   }}")).toMatch(/Empty/);
  });

  it("finds unbalanced brackets and unterminated strings inside an expression", () => {
    expect(checkTemplate("{{ (x + 1 }}")).toMatch(/Unclosed "\("/);
    expect(checkTemplate("{{ x + 1) }}")).toMatch(/Unbalanced "\)"/);
    expect(checkTemplate("{{ [1, 2) }}")).toMatch(/Unbalanced "\)"/);
    expect(checkTemplate('{{ "abc }}')).toMatch(/Unterminated string/);
  });

  it("reports the first problem only, and ignores later well-formed templates", () => {
    expect(checkTemplate("{{ (x }} {{ y }}")).toMatch(/Unclosed "\("/);
    expect(checkTemplate("{{ x }} {{ (y }}")).toMatch(/Unclosed "\("/);
  });

  it("asks a number field for a number or a template, and a text field for nothing", () => {
    expect(checkValueText("100", "number")).toBeNull();
    expect(checkValueText("{{ x + 10 }}", "number")).toBeNull();
    expect(checkValueText("abc", "number")).toMatch(/number/);
    expect(checkValueText("", "number")).toMatch(/number/);
    expect(checkValueText("{{ (x }}", "number")).toMatch(/Unclosed/);
    expect(checkValueText("abc", "text")).toBeNull();
    expect(checkValueText("abc", "auto")).toBeNull();
    expect(checkValueText("{{ (x }}", "text")).toMatch(/Unclosed/);
  });
});

describe("inserting a variable", () => {
  it("replaces the value of a number field (it holds one value)", () => {
    expect(withReference("100", "step", "number")).toBe("{{ step }}");
    expect(withReference("", "step", "number")).toBe("{{ step }}");
  });

  it("appends to the text of a text field", () => {
    expect(withReference("hello ", "name", "text")).toBe("hello {{ name }}");
    expect(withReference("", "name", "text")).toBe("{{ name }}");
    expect(withReference("x", "n", "auto")).toBe("x{{ n }}");
  });
});
