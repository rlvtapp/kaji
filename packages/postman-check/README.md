# Editable Postman export checker

This composite GitHub Action validates Collection 2.1 exports without sending API
requests or evaluating collection scripts. It uses Python 3.12 and jsonschema
4.23.0 against the checked-in official Postman schema. `check.py`, `action.yml`
and the schema can be copied into your own `.github/actions/postman-check` folder.
Review source and pin a verified commit when referencing this repository remotely.

```yaml
- uses: ./.github/actions/postman-check
  with:
    collection: generated/postman/collection.json
    environment: generated/postman/environment.json
```

Both paths are relative to the checkout. The environment input is optional;
credential variables of type `secret` must have blank values. Request IDs must
be present and unique. Paths cannot escape through symlinks or traversal. Failure
messages omit instance values. Passing validation verifies collection shape,
not endpoint availability, API authorization, or multi-request scenarios.

For local verification with jsonschema installed:

```sh
KAJI_COLLECTION=generated/postman/collection.json \
KAJI_ENVIRONMENT=generated/postman/environment.json \
python3 packages/postman-check/check.py
python3 -m unittest discover -s packages/postman-check/test -v
```

The schema comes from
[Postman's official Collection 2.1 endpoint](https://schema.postman.com/json/collection/v2.1.0/collection.json).
Its duplicate in the generator tests verifies generation against the same format.
