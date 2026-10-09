# Input and output interfaces

← [Rust plugins](README.md)

## Input provider

Implement `InputPlugin` to identify your format and load a source into an
`InputContract`. Publish the whole protocol contract and, optionally, blocks.
Register the provider with `InputRegistry` and select it through
`InputProvider<C>`.

[Complete provider example](../../reference/inputs/input-plugins.md#select-or-replace-a-provider)

## Output plugin

Implement `Plugin<L>` for the language you target.

| Method | Responsibility |
| --- | --- |
| `kind`, `meta` | Stable diagnostics and instance identity |
| `requires`, `provides` | Declare contracts read and published |
| `generate` | Read declared inputs, emit files, publish outputs |
| `supports_native_input` | Allow generation without legacy HTTP context |
| `phase` | Select Generate or Post |

Use `cx.files` for owned files and `cx.workspace` for language-level dependencies,
exports and package assembly. Use actual symbols published by other plugins
instead of guessing their names or module paths.

## New language

Implement `Language` with its settings, workspace and finalization hooks. Generate
plugins run before language finalization; Post plugins run after it.

[Executable minimal plugin](../../../examples/custom-plugin/README.md) ·
[Full interface reference](../../internals/typed-plugins.md) · [Lifecycle](../../internals/lifecycle.md)
