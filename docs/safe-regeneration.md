# Safe regeneration

Kaji records generated paths, persistent owner identities, and SHA-256 fingerprints in `.kaji/ownership.json`. Keep this file with the generated SDK when using regeneration in CI. The separate generation replay lock remains `.kaji/generation.lock.json`.

Run `kaji generate --check` to compare output without writing destination files. Add `--format json` for an object with `added`, `modified`, and `removed` path arrays. A nonempty report exits unsuccessfully. Both options work with `--config` and direct generation.

Normal generation validates the full output before writing. It refuses to overwrite locally edited generated code or delete edited obsolete files. Restore a generated file or move its customization into a custom module before regenerating. Files added with `insert_custom` are created once and then preserved. Existing npm manifests retain user scripts, dependencies, and custom settings under the manifest merge rules.

Only fingerprint-matching files in the ownership record can be removed. Unowned files are preserved. Older output can be adopted when its bytes already match, its first eight lines contain a Kaji generation notice, or it is the known replay lock; npm manifests use the merge policy. Unmarked, changed legacy files require manual review rather than automatic adoption. Legacy obsolete files are not removed until ownership has been established.

Plugins can assign a stable package/plugin identity with `GeneratedTree::set_owner`. Composition code must use `into_owned_files` when relocating files to retain this metadata. `into_files` remains available for existing callers, but deliberately omits owner identities. A file without an explicit identity uses a stable path-based package fallback.

Output paths and persisted manifest entries must be relative and cannot contain parent traversal. Symlinks within output paths and a symlink output root are rejected. Preflight prevents validation errors from partially writing output; materialization is not an atomic directory transaction, so an I/O failure during writing can still leave a partial update.
