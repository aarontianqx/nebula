# tap DSL Reference

tap uses YAML as its DSL for defining macros (a `MacroDocument`). This is the
canonical on-disk format; it round-trips losslessly through save/load and
import/export.

## Syntax note: actions and conditions are YAML tags

Actions and conditions are **YAML tagged values** — the variant name is written
as a tag (`!click`, `!pixel_color`), not as a nested map key. This is the single
most important rule:

```yaml
# Correct — tagged
action: !click
  x: 100
  y: 200

# WRONG — rejected with "expected a YAML tag starting with '!'"
action:
  click:
    x: 100
    y: 200
```

Unit variants that carry no fields are written as a bare scalar (no `!`):
`exit`, `always`, `never`.

## Document structure

```yaml
name: My Macro            # required
description: ...          # optional
version: "1.0"            # optional (default "1.0")
author: user              # optional
tags: [example, demo]     # optional

variables: {}             # optional, parameterization
target_window: {}         # optional, window binding
timeline: []              # required, the action sequence
run: {}                   # optional, execution config
```

### Metadata fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Name of the macro |
| `description` | string | No | Human-readable description |
| `version` | string | No | DSL schema version (default `"1.0"`) |
| `author` | string | No | Author name |
| `tags` | string[] | No | Free-form tags for organization/filtering |

## Scope: DSL vs tool modes

This DSL describes **timed macros** (a `timeline` executed by the engine).
Some app features are **event-driven tool modes** (driven by live global input,
not a timeline) and are intentionally **out of scope** — they cannot be
expressed or imported as YAML.

Example: **Key→Click** — while enabled, holding any `A`-`Z` key triggers repeated
clicks; `Space` stops (see `key-to-click-mode.md`).

## Variables

Variables parameterize a macro. They are defined in `variables` and referenced
with `{{ name }}` inside action value fields.

```yaml
variables:
  username:
    type: string      # string | number | boolean
    default: ""
    description: "Username to enter"
  click_x:
    type: number
    default: 100
  enabled:
    type: boolean
    default: true
```

| Type | Description | Default |
|------|-------------|---------|
| `string` | Text value | `""` |
| `number` | Integer or float | `0` |
| `boolean` | True/false | `false` |

### References and expressions

Value fields (coordinates, text, …) accept a literal or a `{{ ... }}` template.
Expressions use [Rhai](https://rhai.rs) syntax, evaluated in a sandbox (no file
or network access, bounded depth/operations):

```yaml
timeline:
  - at_ms: 0
    action: !click
      x: "{{ base_x + offset }}"
      y: "{{ base_y * 2 }}"
  - at_ms: 500
    action: !text_input
      text: "{{ \"Hello, \" + name }}"
```

A value left unresolved (an unfilled variable in a coordinate) is collected by
the **run-before variable form** prompt; macros carrying variables are edited in
the **Code (YAML)** view, since the visual editor projects a lossy resolved view.

## Target window

Bind execution to a specific window:

```yaml
target_window:
  title: "Notepad"            # window title (partial match)
  process: "notepad.exe"      # process name (partial match)
  pause_when_unfocused: true  # pause when the window loses focus
```

Specify `title`, `process`, or both. Omit `target_window` to run on any window.
On macOS, reading the window **title** requires Screen Recording permission;
matching by **process** works without it.

## Timeline

A sequence of timed actions:

```yaml
timeline:
  - at_ms: 0
    action: !click
      x: 100
      y: 200
    enabled: true
    note: "Click the button"
  - at_ms: 500
    action: !wait
      ms: 100
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `at_ms` | number | Yes | Milliseconds since timeline start |
| `action` | tagged value | Yes | The action to perform |
| `enabled` | boolean | No | Whether the action runs (default `true`) |
| `note` | string | No | User comment |

## Actions

### Coordinate system

All `x`/`y` are in the OS **injection space**, shared by recording, playback and
the picker: **physical pixels on Windows** (the app opts into Per-Monitor V2 DPI
awareness) and **points on macOS** (the window server handles Retina scaling).
Because the space is OS- and display-specific, macros are **not portable across
machines** without re-capturing coordinates.

### Mouse

```yaml
action: !click
  x: 100
  y: 200
  button: left        # left | right | middle (default left)
```

```yaml
action: !double_click
  x: 100
  y: 200
  button: left
```

```yaml
action: !mouse_down
  x: 100
  y: 200
  button: left

action: !mouse_up
  x: 100
  y: 200
  button: left
```

```yaml
action: !mouse_move
  x: 100
  y: 200
```

```yaml
action: !drag
  from_x: 100
  from_y: 200
  to_x: 300
  to_y: 400
  duration_ms: 500    # default 500
```

The engine presses at `from`, interpolates toward `to` over `duration_ms`
(~60 steps/sec, capped), then releases at `to`. The interpolation is
interruptible: a Stop/EmergencyStop during the drag releases the button before
halting, so a half-finished drag never leaves the mouse stuck down.

```yaml
action: !scroll
  delta_x: 0
  delta_y: -120       # negative = scroll up
```

### Keyboard

```yaml
action: !key_tap
  key: "Enter"
```

Common key names: `Space`, `Enter`, `Tab`, `Escape`, `Backspace`, `Delete`,
`Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`, `PageDown`, `F1`-`F12`,
`A`-`Z`, `0`-`9`, and modifiers `Ctrl`, `Shift`, `Alt`, `Meta`. Recorded/captured
key names are normalized to these canonical tokens, so a macro recorded on one OS
replays on another.

```yaml
action: !key_down
  key: "Ctrl"

action: !key_up
  key: "Ctrl"
```

```yaml
action: !text_input
  text: "Hello, World!"
```

### Timing

```yaml
action: !wait
  ms: 1000
```

```yaml
action: !wait_until
  condition: !pixel_color
    x: 100
    y: 100
    color: "#00FF00"
    tolerance: 10
  timeout_ms: 5000        # optional; omit = wait forever
  poll_interval_ms: 100   # default 100
```

### Conditional

`then_action`/`else_action` are themselves actions (a tag, or bare `exit`):

```yaml
action: !conditional
  condition: !counter
    key: "loop_count"
    op: "<"
    value: 10
  then_action: !click
    x: 100
    y: 200
  else_action: exit       # optional
```

### Counters

```yaml
action: !set_counter
  key: "count"
  value: 10

action: !incr_counter
  key: "count"

action: !decr_counter
  key: "count"

action: !reset_counter
  key: "count"
```

### Control flow

```yaml
action: exit              # stop the macro (bare scalar, no fields)
```

```yaml
action: !call_macro
  name: "login_sequence"
  args:
    username: "{{ user }}"
    password: "{{ pass }}"
```

`call_macro` loads another saved macro by `name` and expands its timeline in
place at runtime. `args` override the child's same-named variables and may
reference the parent scope via `{{ ... }}`. The child runs in an isolated scope
(its variable/counter writes do not leak back), a child `exit` only ends the
child, and cycles or calls deeper than 10 levels are skipped with an
`EngineEvent::Error` rather than hanging the run.

## Conditions

Used by `wait_until` and `conditional`.

```yaml
condition: !window_focused
  title: "Notepad"
  process: "notepad.exe"
```

```yaml
condition: !window_exists
  title: "Calculator"
```

```yaml
condition: !pixel_color
  x: 100
  y: 100
  color: "#FF0000"
  tolerance: 10           # default 10
```

```yaml
condition: !counter
  key: "loop_count"
  op: "<"                 # == != > < >= <=
  value: 10
```

### Logical operators

`always`/`never` are bare scalars; `and`/`or` take a sequence; `not` nests the
inner condition under `condition:`.

```yaml
condition: always
condition: never
```

```yaml
condition: !and
  - !window_focused
    title: "Notepad"
  - !pixel_color
    x: 100
    y: 100
    color: "#00FF00"
```

```yaml
condition: !or
  - !window_focused
    title: "Notepad"
  - !window_focused
    title: "Calculator"
```

```yaml
condition: !not
  condition: !window_focused
    title: "Notepad"
```

## Run configuration

```yaml
run:
  repeat: 1            # number of times (0 = forever)
  start_delay_ms: 3000 # countdown before the first action
  speed: 1.0           # speed multiplier (0.5 = half, 2.0 = double)
```

## Complete example

```yaml
name: Auto Login
description: Automatically logs into an application
version: "1.0"
author: user

variables:
  username:
    type: string
    default: ""
    description: "Login username"
  password:
    type: string
    default: ""
    description: "Login password"
  button_x:
    type: number
    default: 640

target_window:
  title: "Login"
  pause_when_unfocused: true

timeline:
  - at_ms: 0
    action: !click
      x: 500
      y: 300
    note: "Click username field"
  - at_ms: 200
    action: !text_input
      text: "{{ username }}"
  - at_ms: 700
    action: !click
      x: 500
      y: 350
    note: "Click password field"
  - at_ms: 900
    action: !text_input
      text: "{{ password }}"
  - at_ms: 1400
    action: !click
      x: "{{ button_x }}"
      y: 400
    note: "Click login button"
  - at_ms: 2000
    action: !wait_until
      condition: !window_exists
        title: "Dashboard"
      timeout_ms: 10000
    note: "Wait for login to complete"

run:
  repeat: 1
  start_delay_ms: 3000
  speed: 1.0
```

## Validation

YAML is validated on import:

- Required fields present; types match (numbers for coordinates, etc.)
- Coordinates within range (-100000 to 100000)
- Variable names are valid identifiers
- Colors in `#RRGGBB` format
- Comparison operators are valid

Validation errors include the field path and a descriptive message.

> See `tap/templates/*.yaml` for working, parse-validated examples.
