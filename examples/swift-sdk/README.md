# Swift SDK

Generate a Swift Package Manager SDK from an OpenAPI document:

```sh
poolster generate ../openapi.yaml --output generated --language swift
cd generated/swift
swift build
```

The generated package has no runtime package dependencies. It uses Foundation
`URLSession`, `Codable`, and Poolster's small `JSONValue` type for unconstrained
JSON schemas.

```swift
import ExampleSdk

let client = PoolsterClient(
    options: .init(
        baseURL: URL(string: "https://api.example.com")!,
        headers: ["Authorization": "Bearer " + (ProcessInfo.processInfo.environment["API_KEY"] ?? "")]
    )
)

let pets = try await client.listPets()
```

Pass a custom `URLSession` to configure connection policy, and implement
`PoolsterClientHook` for request/response telemetry.
