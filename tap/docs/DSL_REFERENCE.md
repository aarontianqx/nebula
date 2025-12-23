# TAP DSL Reference

TAP uses YAML as its Domain Specific Language (DSL) for defining macros. This document provides a complete reference for the DSL syntax.

## Overview

A TAP macro file has the following structure:

```yaml
name: My Macro
description: Optional description
version: "1.0"
author: user

variables:
  # Parameterized variables

target_window:
  # Window binding

timeline:
  # Action sequence

run:
  # Execution configuration
```

## Metadata Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Name of the macro |
| `description` | string | No | Human-readable description |
| `version` | string | No | DSL schema version (default: "1.0") |
| `author` | string | No | Author name |

## Variables

Variables allow parameterization of macros. They are defined in the `variables` section and referenced using `{{ var_name }}` syntax.

```yaml
variables:
  username:
    type: string
    default: ""
    description: "Username to enter"
  click_x:
    type: number
    default: 100
  enabled:
    type: boolean
    default: true
```

### Variable Types

| Type | Description | Default Value |
|------|-------------|---------------|
| `string` | Text value | `""` |
| `number` | Numeric value (integer or float) | `0` |
| `boolean` | True/false value | `false` |

### Variable References

Variables can be referenced in action parameters:

```yaml
timeline:
  - at_ms: 0
    action:
      click:
        x: "{{ click_x }}"
        y: "{{ click_y }}"
  - at_ms: 500
    action:
      text_input:
        text: "{{ username }}"
```

### Expressions

Complex expressions are supported using Rhai syntax:

```yaml
timeline:
  - at_ms: 0
    action:
      click:
        x: "{{ base_x + offset }}"
        y: "{{ base_y * 2 }}"
  - at_ms: 500
    action:
      text_input:
        text: "{{ \"Hello, \" + name }}"
```

## Target Window

Bind macro execution to a specific window:

```yaml
target_window:
  title: "Notepad"           # Window title (partial match)
  process: "notepad.exe"     # Process name (partial match)
  pause_when_unfocused: true # Pause when window loses focus
```

Either `title` or `process` (or both) can be specified. Leave empty to run on any window.

## Timeline

The timeline is a sequence of timed actions:

```yaml
timeline:
  - at_ms: 0
    action: { click: { x: 100, y: 200 } }
    enabled: true
    note: "Click the button"
  - at_ms: 500
    action: { wait: { ms: 100 } }
```

### Timed Action Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `at_ms` | number | Yes | Milliseconds since timeline start |
| `action` | object | Yes | The action to perform |
| `enabled` | boolean | No | Whether action is enabled (default: true) |
| `note` | string | No | User comment/note |

## Actions

### Mouse Actions

#### click
```yaml
action:
  click:
    x: 100
    y: 200
    button: left  # left, right, middle (default: left)
```

#### double_click
```yaml
action:
  double_click:
    x: 100
    y: 200
    button: left
```

#### mouse_down / mouse_up
```yaml
action:
  mouse_down:
    x: 100
    y: 200
    button: left

action:
  mouse_up:
    x: 100
    y: 200
    button: left
```

#### mouse_move
```yaml
action:
  mouse_move:
    x: 100
    y: 200
```

#### drag
```yaml
action:
  drag:
    from_x: 100
    from_y: 200
    to_x: 300
    to_y: 400
    duration_ms: 500  # default: 500
```

#### scroll
```yaml
action:
  scroll:
    delta_x: 0
    delta_y: -120  # negative = scroll up
```

### Keyboard Actions

#### key_tap
```yaml
action:
  key_tap:
    key: "Enter"  # Key name
```

Common key names: `Space`, `Enter`, `Tab`, `Escape`, `Backspace`, `Delete`, `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`, `PageDown`, `F1`-`F12`, `A`-`Z`, `0`-`9`

#### key_down / key_up
```yaml
action:
  key_down:
    key: "Control"

action:
  key_up:
    key: "Control"
```

#### text_input
```yaml
action:
  text_input:
    text: "Hello, World!"
```

### Timing Actions

#### wait
```yaml
action:
  wait:
    ms: 1000
```

#### wait_until
Wait for a condition to be satisfied:

```yaml
action:
  wait_until:
    condition:
      pixel_color:
        x: 100
        y: 100
        color: "#00FF00"
        tolerance: 10
    timeout_ms: 5000      # optional, null = wait forever
    poll_interval_ms: 100 # default: 100
```

### Conditional Actions

#### conditional
```yaml
action:
  conditional:
    condition:
      counter:
        key: "loop_count"
        op: "<"
        value: 10
    then_action:
      click: { x: 100, y: 200 }
    else_action:  # optional
      exit: {}
```

### Counter Actions

#### set_counter
```yaml
action:
  set_counter:
    key: "count"
    value: 10
```

#### incr_counter / decr_counter
```yaml
action:
  incr_counter:
    key: "count"

action:
  decr_counter:
    key: "count"
```

#### reset_counter
```yaml
action:
  reset_counter:
    key: "count"
```

### Control Flow

#### exit
Stop macro execution:

```yaml
action:
  exit: {}
```

#### call_macro
Call another saved macro:

```yaml
action:
  call_macro:
    name: "login_sequence"
    args:
      username: "{{ user }}"
      password: "{{ pass }}"
```

## Conditions

Conditions are used in `wait_until` and `conditional` actions.

### window_focused
```yaml
condition:
  window_focused:
    title: "Notepad"
    process: "notepad.exe"
```

### window_exists
```yaml
condition:
  window_exists:
    title: "Calculator"
```

### pixel_color
```yaml
condition:
  pixel_color:
    x: 100
    y: 100
    color: "#FF0000"
    tolerance: 10  # default: 10
```

### counter
```yaml
condition:
  counter:
    key: "loop_count"
    op: "<"     # ==, !=, >, <, >=, <=
    value: 10
```

### Logical Operators

#### always / never
```yaml
condition:
  always: {}

condition:
  never: {}
```

#### and
```yaml
condition:
  and:
    - window_focused: { title: "Notepad" }
    - pixel_color: { x: 100, y: 100, color: "#00FF00" }
```

#### or
```yaml
condition:
  or:
    - window_focused: { title: "Notepad" }
    - window_focused: { title: "Calculator" }
```

#### not
```yaml
condition:
  not:
    window_focused: { title: "Notepad" }
```

## Run Configuration

```yaml
run:
  repeat: 1           # Number of times (0 = forever)
  start_delay_ms: 3000 # Countdown before start
  speed: 1.0          # Speed multiplier (0.5 = half speed, 2.0 = double speed)
```

## Complete Example

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
  button_y:
    type: number
    default: 400

target_window:
  title: "Login"
  pause_when_unfocused: true

timeline:
  - at_ms: 0
    action:
      click:
        x: 500
        y: 300
    note: "Click username field"
  
  - at_ms: 200
    action:
      text_input:
        text: "{{ username }}"
  
  - at_ms: 700
    action:
      click:
        x: 500
        y: 350
    note: "Click password field"
  
  - at_ms: 900
    action:
      text_input:
        text: "{{ password }}"
  
  - at_ms: 1400
    action:
      click:
        x: "{{ button_x }}"
        y: "{{ button_y }}"
    note: "Click login button"
  
  - at_ms: 2000
    action:
      wait_until:
        condition:
          window_exists:
            title: "Dashboard"
        timeout_ms: 10000
    note: "Wait for login to complete"

run:
  repeat: 1
  start_delay_ms: 3000
  speed: 1.0
```

## Validation

TAP validates YAML files on import:

- Required fields must be present
- Types must match (numbers for coordinates, etc.)
- Coordinates must be within reasonable range (-100000 to 100000)
- Variable names must be valid identifiers
- Colors must be in `#RRGGBB` format
- Comparison operators must be valid

Validation errors include the field path and a descriptive message.

