---
description: The console methods Incarnative.js provides, format specifiers, output and colour, and where printing differs from Node.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Console <Badge type="warning" text="unreleased" />

**Incarnative.js** provides a built-in `console` object. It is available in
your app without an import.

The `console` object follows the
[WHATWG Console Standard](https://console.spec.whatwg.org/). Where the
standard leaves the format open, such as how values print, the host
follows Node's `util.inspect`.

## Methods

| Method | Notes |
| --- | --- |
| `log`, `debug`, `dirxml` | Print the arguments, separated by a space. `debug` is grey. |
| `info`, `warn`, `error` | Same, behind `ℹ`, `⚠` and `✖`. |
| `assert` | Prints `Assertion failed` behind `✖` when the condition is falsy. A string message follows after `: `. |
| `trace` | Prints `Trace`, the arguments and the caller's stack. |
| `dir` | Prints one value. A string is quoted. |
| `table` | Draws an array or object as a table. The second argument picks the columns. |
| `count`, `countReset` | Count per label. `default` is the label when omitted. |
| `time`, `timeLog`, `timeEnd` | Print `label: 1.234ms`. A missing or duplicate timer warns. |
| `group`, `groupCollapsed`, `groupEnd` | Indent what follows. The label is marked `▼` or `▶`. |
| `clear` | Closes every open group. |

`profile`, `profileEnd` and `timeStamp` are not provided.

## Format specifiers

When the first argument is a string and more arguments follow, these
specifiers take the next argument:

| Specifier | Prints |
| --- | --- |
| `%s` | The argument as a string |
| `%d`, `%i` | The integer at the start of the argument, or `NaN` |
| `%f` | The decimal number at the start of the argument, or `NaN` |
| `%o`, `%O` | The argument as `console.dir` prints it |
| `%c` | Nothing. Takes its argument |
| `%%` | A `%` |

```js
console.log("%s is %d years", "Ann", 42.9);
```

```
Ann is 42 years
```

## Output

Every method writes to the host's stderr.

| Where | What happens |
| --- | --- |
| The development window | Shows the output in the terminal that ran `inca dev` |
| A packaged app | Has no readable stderr, so the output is not visible |

A line starts with a marker:

| Marker | Starts |
| --- | --- |
| `✖`{.marker-error} | An `error` line or a failed `assert` |
| `⚠`{.marker-warn} | A `warn` line, or a warning from a counter or timer |
| `ℹ`{.marker-info} | An `info` line |
| `│ ` | Every line inside an open group, once per level |

## Colour

Output is coloured when stderr is a terminal, and values are coloured by
type as in Node. These environment variables override the terminal check:

| Variable | Effect |
| --- | --- |
| `NO_COLOR` | A non-empty value turns colour off. It wins over `FORCE_COLOR`. |
| `FORCE_COLOR` | `0` or `false` turns colour off. Any other non-empty value turns it on. An empty value is ignored. |

## Differences from Node

- `%d` and `%i` parse an integer where Node converts the whole value to a
  number. `%s` prints an object's string form where Node inspects it.

## Known issues

- Output is not wrapped across lines and is not cut after 100 items.
- A property getter runs when its object is printed.
- A class instance prints without its class name. Symbol-keyed
  properties and extra properties on arrays and errors are dropped. A
  `Promise`, a typed array and a `WeakMap` print as plain objects. A
  circular reference prints `[Object]` at the depth limit, with no marker.
- A `Date`, `RegExp`, `Map` or `Set` prints like Node, but properties
  added to it and the name of a subclass are left out.
- A string inside an array or object is always single-quoted.
- `%o` and `%O` print alike, and `%c` drops its styling.
- Timers always print milliseconds.
