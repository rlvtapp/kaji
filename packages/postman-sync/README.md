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
