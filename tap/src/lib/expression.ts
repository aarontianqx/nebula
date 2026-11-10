// Text <-> `DslValue` for the value fields, and a cheap lint for `{{ expression }}` templates.
//
// The backend evaluates templates (Rhai, sandboxed) only while a run resolves its actions, so a
// mistake there surfaces as a run-time error. The lint here catches the common typing slips
// (unclosed `{{`, unbalanced brackets, an unterminated string) while the user is still editing.
// It is advisory: it never blocks an edit, and it does not try to parse Rhai.

import type { DslValue } from "./types";

/**
 * How a field's text becomes a [`DslValue`]:
 *
 * - `number`: coordinates, deltas, counter values. A numeric literal is stored as a number;
 *   anything else (an expression, or text still being typed) as the string typed.
 * - `text`: typed text. Always a string, so `42` stays the text "42".
 * - `auto`: `call_macro` arguments. A numeric literal is a number, `true` / `false` a boolean,
 *   anything else a string; the callee's variables are typed by this.
 */
export type ValueMode = "number" | "text" | "auto";

const NUMERIC_LITERAL = /^-?\d+(\.\d+)?$/;

export function valueToText(value: DslValue): string {
  return String(value);
}

export function parseValueText(text: string, mode: ValueMode): DslValue {
  if (mode === "text") return text;
  const trimmed = text.trim();
  if (NUMERIC_LITERAL.test(trimmed)) return Number(trimmed);
  if (mode === "auto" && (trimmed === "true" || trimmed === "false")) return trimmed === "true";
  return text;
}

/** Whether the text contains a template (`{{ ... }}`) rather than being a plain literal. */
export function hasTemplate(text: string): boolean {
  return text.includes("{{");
}

const CLOSERS: Record<string, string> = { ")": "(", "]": "[", "}": "{" };
const OPENERS = new Set(Object.values(CLOSERS));
const QUOTES = new Set(['"', "'", "`"]);

/** The first problem inside one expression, or `null`. Brackets and strings only. */
function checkExpression(expression: string): string | null {
  if (expression.trim() === "") return "Empty {{ }}: write an expression or a variable name.";

  const open: string[] = [];
  let quote: string | null = null;
  for (let i = 0; i < expression.length; i += 1) {
    const ch = expression[i];
    if (quote) {
      if (ch === "\\" && quote !== "`") i += 1;
      else if (ch === quote) quote = null;
    } else if (QUOTES.has(ch)) {
      quote = ch;
    } else if (OPENERS.has(ch)) {
      open.push(ch);
    } else if (ch in CLOSERS) {
      if (open.pop() !== CLOSERS[ch]) return `Unbalanced "${ch}" in the expression.`;
    }
  }
  if (quote) return `Unterminated string (missing ${quote}).`;
  if (open.length > 0) return `Unclosed "${open[open.length - 1]}" in the expression.`;
  return null;
}

/**
 * The first problem in the templates of `text`, or `null`. Mirrors how the backend splits a
 * template: an expression runs from `{{` to the first `}}` after it.
 */
export function checkTemplate(text: string): string | null {
  let from = 0;
  for (;;) {
    const start = text.indexOf("{{", from);
    if (start === -1) return null;
    const end = text.indexOf("}}", start + 2);
    if (end === -1) return "Missing closing }} for {{.";
    const problem = checkExpression(text.slice(start + 2, end));
    if (problem) return problem;
    from = end + 2;
  }
}

/**
 * The advice shown under a value field, or `null` when the text looks fine. A `number` field also
 * wants either a number or a template (the backend rejects anything else for a coordinate).
 */
export function checkValueText(text: string, mode: ValueMode): string | null {
  if (hasTemplate(text)) return checkTemplate(text);
  if (mode === "number" && !NUMERIC_LITERAL.test(text.trim())) {
    return "Enter a number, or an expression like {{ x + 10 }}.";
  }
  return null;
}

/**
 * `text` with a reference to `name` in it. A number field holds a single value, so the reference
 * replaces it; a text field keeps what was typed and gets the reference appended.
 */
export function withReference(text: string, name: string, mode: ValueMode): string {
  const reference = `{{ ${name} }}`;
  return mode === "number" || text === "" ? reference : `${text}${reference}`;
}
