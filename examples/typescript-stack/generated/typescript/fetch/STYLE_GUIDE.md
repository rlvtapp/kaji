# Poolster Pets TypeScript SDK style guide

This package selected the **namespaced instantiated client** surface. Generated models and direct operation exports are available in every mode.

- `ts::sdk().raw()`: direct models and operation functions only.
- `ts::sdk().flat()`: `client.createContact(...)`.
- `ts::sdk().namespaced()`: `client.contacts.create(...)`.

Configure schema rendering with `ts::sdk().model_options(ts::ModelOptions { ..Default::default() })`. Select Fetch or Axios through `.fetch()` or `.axios()`.
