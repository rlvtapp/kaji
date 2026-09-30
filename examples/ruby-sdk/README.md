# Ruby SDK

Generate an installable Ruby 3.1+ gem from an OpenAPI document:

```sh
kaji generate ../openapi.yaml --output generated --language ruby
cd generated/ruby
gem build *.gemspec
```

The generated client has no HTTP-gem dependency. It uses `Net::HTTP`, `URI`,
and `JSON` from Ruby's standard library.

```ruby
require "example_api_sdk"

client = ExampleApiSdk::Client.new(
  base_url: "https://api.example.com",
  api_key: ENV.fetch("API_KEY", nil),
)

# Both surfaces are emitted with the default namespaced style.
client.contacts.get_contact(contact_id: "contact_123")
client.get_contact(contact_id: "contact_123")
```

Use `--client-style flat` when you only want direct operation methods.
