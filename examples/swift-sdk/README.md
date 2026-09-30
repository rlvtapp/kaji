# Swift SDK

Generate a Swift Package Manager SDK from an OpenAPI document:

```sh
kaji generate ../openapi.yaml --output generated --language swift
cd generated/swift
swift build
```

The generated package has no runtime package dependencies. It uses Foundation
`URLSession`, `Codable`, and Kaji's small `JSONValue` type for unconstrained
JSON schemas.

```swift
import ExampleSdk

let client = KajiClient(
    options: .init(
        baseURL: URL(string: "https://api.example.com")!,
        headers: ["Authorization": "Bearer " + (ProcessInfo.processInfo.environment["API_KEY"] ?? "")]
    )
)

let pets = try await client.listPets()
```

Pass a custom `URLSession` to configure connection policy, and implement
`KajiClientHook` for request/response telemetry.
