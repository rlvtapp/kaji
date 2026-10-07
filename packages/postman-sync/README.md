# Synchronize a generated Postman collection

This editable Node helper compares a generated Collection 2.1 document with an
explicit existing collection UID. It defaults to read-only `check`, reports
SHA-256 fingerprints and exits nonzero on drift. It does not create collections,
delete collections or synchronize environment secrets.

```sh
KAJI_POSTMAN_COLLECTION=generated/postman/api.postman_collection.json \
KAJI_POSTMAN_UID=YOUR_COLLECTION_UID \
KAJI_POSTMAN_API_KEY=YOUR_SECRET \
node packages/postman-sync/sync.mjs
```

After reviewing both documents, use `KAJI_POSTMAN_MODE=publish` and set
`KAJI_POSTMAN_EXPECTED_HASH` to the check's `remoteHash`. Changed publication
requires this hash, checks it again, replaces the collection and reads it back.
The Postman API provides no atomic compare-and-swap here: avoid simultaneous
manual edits or publishing jobs between the check and update. A read-back failure
means the publication is unverified; an update may already have happened.

The composite action exposes the same inputs. Pass `api-key` from a secret and
protect publication with your repository's release environment. Run the existing
official-schema checker before either mode. Mocked helper tests cover drift,
manual edits, redacted failures and read-back verification; no live Postman account
has been accessed by this implementation.

API requests use the [official Postman API](https://learning.postman.com/docs/reference/postman-api/intro-api).

```sh
node --test packages/postman-sync/test/*.mjs
```

## Existing environment sync

Use `node packages/postman-sync/environment.mjs` with `KAJI_POSTMAN_ENVIRONMENT`,
`KAJI_POSTMAN_UID`, `KAJI_POSTMAN_API_KEY`, and the same `KAJI_POSTMAN_MODE` /
`KAJI_POSTMAN_EXPECTED_HASH` review flow as collections. The composite action is
`./packages/postman-sync/environment` with `environment`, `uid`, `api-key`,
`mode`, and `expected-hash` inputs.

The destination must already exist. The check reports hashes only. Publishing
updates portable environment fields through Postman's [replace environment
endpoint](https://learning.postman.com/api-docs/api-reference/environments/put-environment)
and verifies a fresh read-back. Source secret variables must be empty placeholders;
remote secret values and remote variables absent from the source are preserved.
A remote secret cannot be downgraded or replaced by a source value. Service identity
and export metadata are not submitted. This manages shared variables only; Postman
local values and Vault secrets are outside the API's scope. There is no atomic
compare-and-swap in this endpoint, so avoid concurrent edits between review and
publication. Workspace creation, relocation, and removal are not performed.
