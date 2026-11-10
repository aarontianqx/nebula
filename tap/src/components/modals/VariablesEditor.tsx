import { useState } from "react";

import { convertDefault, defaultForType, isValidIdentifier, renameKey, uniqueName, variableEntries } from "../../lib/document";
import type { VariableDefinition, VariableType } from "../../lib/types";
import { useDocumentStore } from "../../stores/documentStore";
import { useUiStore } from "../../stores/uiStore";

const TYPES: VariableType[] = ["string", "number", "boolean"];

/**
 * Defines the variables a macro uses: name, type, default and description. A variable is used as
 * `{{ name }}` in the value fields; the values for a run are set in the "Values" dialog.
 *
 * Renaming a variable does not rewrite the `{{ }}` references to it.
 */
export function VariablesEditor() {
  const show = useUiStore((s) => s.showVariableDefinitions);
  const doc = useDocumentStore((s) => s.doc);

  if (!show) return null;

  const close = () => useUiStore.getState().setShowVariableDefinitions(false);
  const store = useDocumentStore.getState;
  const entries = variableEntries(doc);
  const variables = doc.variables ?? {};

  return (
    <div className="modal-overlay" onClick={close}>
      <div className="modal modal-wide" onClick={(e) => e.stopPropagation()}>
        <h3>Variables</h3>
        <p className="modal-lead">
          Use a variable in any value field as <code>{"{{ name }}"}</code>. Counters need no definition: a counter is
          used by name, too.
        </p>

        {entries.length === 0 ? (
          <div className="field-note">No variables yet.</div>
        ) : (
          <div className="var-def-table">
            <div className="var-def-row var-def-head">
              <span>Name</span>
              <span>Type</span>
              <span>Default</span>
              <span>Description</span>
              <span />
            </div>
            {entries.map(([name, def]) => (
              <VariableRow
                key={name}
                name={name}
                def={def}
                others={entries.map(([n]) => n).filter((n) => n !== name)}
                onChange={(next) => store().setVariables({ ...variables, [name]: next })}
                onRename={(to) => store().setVariables(renameKey(variables, name, to))}
                onRemove={() => store().setVariables(Object.fromEntries(entries.filter(([n]) => n !== name)))}
              />
            ))}
          </div>
        )}

        <div className="modal-actions modal-actions-spread">
          <button
            className="btn btn-sm"
            onClick={() =>
              store().setVariables({
                ...variables,
                [uniqueName("variable", Object.keys(variables))]: { type: "string", default: "" },
              })
            }
          >
            + Add variable
          </button>
          <button className="btn btn-primary" onClick={close}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}

function VariableRow(props: {
  name: string;
  def: VariableDefinition;
  others: string[];
  onChange: (def: VariableDefinition) => void;
  onRename: (to: string) => void;
  onRemove: () => void;
}) {
  const { def } = props;
  // The name is committed on blur: it can be typed through states that are not names yet.
  const [draft, setDraft] = useState(props.name);
  const next = draft.trim();
  const valid = isValidIdentifier(next) && !props.others.includes(next);
  const problem = valid
    ? null
    : props.others.includes(next)
      ? "Another variable has this name."
      : "Use letters, digits and _, and do not start with a digit.";

  function commit(): void {
    if (next !== props.name && valid) props.onRename(next);
    else setDraft(props.name);
  }

  return (
    <div className="var-def-row">
      <input
        type="text"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        className="input"
        aria-label="Variable name"
        aria-invalid={valid ? undefined : true}
        title={problem ?? undefined}
        spellCheck={false}
      />
      <select
        value={def.type}
        onChange={(e) => {
          const type = e.target.value as VariableType;
          props.onChange({ ...def, type, default: convertDefault(def.default, type) });
        }}
        className="input"
        aria-label="Variable type"
      >
        {TYPES.map((t) => (
          <option key={t} value={t}>
            {t}
          </option>
        ))}
      </select>
      <DefaultInput def={def} onChange={(value) => props.onChange({ ...def, default: value })} />
      <input
        type="text"
        value={def.description ?? ""}
        onChange={(e) => props.onChange({ ...def, description: e.target.value === "" ? undefined : e.target.value })}
        className="input"
        placeholder="optional"
        aria-label="Variable description"
      />
      <button className="btn btn-sm" onClick={props.onRemove} title="Remove this variable">
        ×
      </button>
    </div>
  );
}

/** The default value, as an input that matches the variable's type. */
function DefaultInput(props: { def: VariableDefinition; onChange: (value: string | number | boolean) => void }) {
  const value = convertDefault(props.def.default ?? defaultForType(props.def.type), props.def.type);

  if (props.def.type === "boolean") {
    return (
      <label className="checkbox-label">
        <input
          type="checkbox"
          checked={value === true}
          onChange={(e) => props.onChange(e.target.checked)}
          aria-label="Default value"
        />
        {value === true ? "true" : "false"}
      </label>
    );
  }
  if (props.def.type === "number") {
    return (
      <input
        type="number"
        value={value as number}
        onChange={(e) => props.onChange(parseFloat(e.target.value) || 0)}
        className="input"
        aria-label="Default value"
      />
    );
  }
  return (
    <input
      type="text"
      value={value as string}
      onChange={(e) => props.onChange(e.target.value)}
      className="input"
      aria-label="Default value"
    />
  );
}
