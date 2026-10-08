# PHP generator templates

Rust renderers load these files with `include_str!`. Files ending in `.php.tmpl`
contain placeholders and are completed before they become generated PHP source:

| Placeholder | Replaced by |
| --- | --- |
| `__NAMESPACE__` | The generated Composer namespace. |
| `__MODULE__` | The generated package module name used by operation tests. |

`pagination_helpers.php.tmpl` is a method fragment inserted into the generated
client. Native test probes live under `tests/fixtures/` so they are separate
from the source emitted into customer SDKs.
