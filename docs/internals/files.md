# File ownership

← [Internals](README.md)

Plugins emit files into an in-memory package before Poolster writes them.

| API | Use |
| --- | --- |
| Rust `cx.files.emit` / JS `ctx.emitFile` | Regenerated owned files |
| Rust `emit_custom` / JS `preserveExisting: true` | Create-once user-maintained files |
| JS `replaceFile` | Explicit staged replacement retaining ownership |
| `check` / preview | Inspect changes without writing |

Paths stay inside the package/output root. Conflicting emitters and escaping
paths fail. Unrelated files remain untouched; locally edited generated files
are protected by regeneration checks.

Language workspaces assemble dependencies, exports and manifests before files
are finalized. Conflicting dependency declarations are diagnosed rather than
silently choosing a version.

**Next:** [Regenerate in JavaScript](../javascript/regeneration.md) ·
[Check and write in Rust](../rust/files.md) · [Detailed safety rules](../reference/regeneration/safe-regeneration.md)
