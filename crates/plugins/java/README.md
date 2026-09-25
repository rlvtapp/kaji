# Kaji Java plugin

Generates a Java 17+ SDK from Kaji's neutral `Api` model. Generated packages
use `java.net.http.HttpClient` and Jackson, and include both Gradle and Maven
build descriptors so a consumer can choose either build tool.

`generate_java_sdk` preserves the direct-operation client API. Select
`generate_java_sdk_with_style(..., SdkClientStyle::Namespaced)` to additionally
export resource accessors such as `client.contacts().get(input)`.

See [STYLE_GUIDE.md](STYLE_GUIDE.md) for both generated client shapes.
