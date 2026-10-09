# Safe regeneration

Poolster protects generated output with path ownership and SHA-256 fingerprints.
Keep both bookkeeping files with the SDK when regenerating in CI:

| File | Records |
| --- | --- |
| `.poolster/ownership.json` | Generated paths, persistent owners and fingerprints |
| `.poolster/generation.lock.json` | Generation replay lock |

## Preview changes

```sh
poolster generate --config poolster.json --check
poolster generate --config poolster.json --check --format json
```

`--check` compares output without writing destination files. JSON contains
`added`, `modified` and `removed` path arrays. A nonempty report exits
unsuccessfully. Both flags also work with direct generation.

## Resolve ownership conflicts

Normal generation validates the complete output before writing.

| File state | Generation behavior | What to do |
| --- | --- | --- |
| Unchanged owned output | May update or remove it | Regenerate normally |
| Locally edited generated output | Refuses overwrite or obsolete-file deletion | Restore it or move customization to a maintained custom module |
| Unowned file | Preserved | Keep author-maintained files outside generated ownership |
| `insert_custom` file | Created once, then preserved | Edit the custom module normally |
| Existing npm manifest | Merged | User scripts, dependencies and custom settings follow manifest merge rules |

Only fingerprint-matching files in the ownership record can be removed.

## Adopt legacy output

Older output can be adopted when its bytes already match, its first eight lines
contain a Poolster generation notice, or it is the known replay lock. npm manifests
use the merge policy.

Changed, unmarked legacy files require manual review. Legacy obsolete files are
preserved until ownership is established.

## Preserve ownership in plugins

Use `GeneratedTree::set_owner` for a stable package/plugin identity and
`into_owned_files` when relocating files. `into_files` omits owner identities;
files without an explicit identity use a stable path-based package fallback.

## Path and write limits

- Output paths and persisted manifest entries must be relative, without parent traversal.
- Symlinks within output paths and symlink output roots are rejected.
- Preflight catches validation errors before writing.
- Materialization is **not an atomic directory transaction**: an I/O failure
  during writing can leave a partial update.
