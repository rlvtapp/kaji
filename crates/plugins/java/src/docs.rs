use super::*;

pub(super) fn settings_gradle(artifact: &str) -> String {
    format!("rootProject.name = '{artifact}'\n")
}

pub(super) fn build_gradle(package: &str, artifact: &str, version: &str) -> String {
    format!(
        "plugins {{\n    id 'java-library'\n    id 'maven-publish'\n}}\n\ngroup = '{package}'\nversion = '{version}'\n\njava {{\n    toolchain {{\n        languageVersion = JavaLanguageVersion.of(17)\n    }}\n    withSourcesJar()\n}}\n\nrepositories {{\n    mavenCentral()\n}}\n\ndependencies {{\n    api 'com.fasterxml.jackson.core:jackson-databind:2.18.3'\n    api 'com.fasterxml.jackson.datatype:jackson-datatype-jsr310:2.18.3'\n}}\n\npublishing {{\n    publications {{\n        maven(MavenPublication) {{\n            from components.java\n            artifactId = '{artifact}'\n        }}\n    }}\n}}\n"
    )
}

pub(super) fn pom_xml(package: &str, artifact: &str, version: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd\">\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>{package}</groupId>\n  <artifactId>{artifact}</artifactId>\n  <version>{version}</version>\n  <properties>\n    <maven.compiler.release>17</maven.compiler.release>\n    <maven.compiler.source>17</maven.compiler.source>\n    <maven.compiler.target>17</maven.compiler.target>\n    <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>\n  </properties>\n  <dependencies>\n    <dependency><groupId>com.fasterxml.jackson.core</groupId><artifactId>jackson-databind</artifactId><version>2.18.3</version></dependency>\n    <dependency><groupId>com.fasterxml.jackson.datatype</groupId><artifactId>jackson-datatype-jsr310</artifactId><version>2.18.3</version></dependency>\n  </dependencies>\n</project>\n"
    )
}

pub(super) fn readme(api: &Api, package: &str, artifact: &str, style: SdkClientStyle) -> String {
    let call = match style {
        SdkClientStyle::Flat => "client.getContact(new Client.GetContactRequest(id));",
        SdkClientStyle::Namespaced => "client.contacts().get(new Client.GetContactRequest(id));",
    };
    let middleware = r#"## Runtime customization

Supply a customer-owned `java.net.http.HttpClient` in `ClientConfig`:

```java
var config = new ClientConfig(
    "https://api.example.com", System.getenv("API_KEY"),
    "Authorization", "Bearer", java.util.Map.of(), customerHttpClient,
    java.time.Duration.ofSeconds(30), RetryConfig.defaults(), null);
var client = new Client(config);
```

`customerHttpClient` can be a delegating `HttpClient` subclass. Implement its
abstract methods, forward client configuration and both `sendAsync` overloads,
and wrap `send(request, bodyHandler)` (the path used by this SDK). Rebuild an
immutable request with `HttpRequest.newBuilder(request, (name, value) -> true)`
to change headers, method, URI or body before forwarding. Return a replacement
`HttpResponse<T>` to rewrite a response or short circuit, preserving the supplied
body handler's `T` representation. Catch and translate transport failures while
preserving interruption. Observer hooks do not return replacement requests or
responses. The decorator sees each transport attempt, including SDK retries;
keep rewrites repeatable and avoid consuming streaming bodies during inspection.
"#;
    format!(
        "# {artifact}\n\nGenerated Java 17+ SDK for {}. See [STYLE_GUIDE.md](STYLE_GUIDE.md) for the selected public API.\n\n```java\nimport {package}.Client;\nimport {package}.ClientConfig;\n\nvar client = new Client(new ClientConfig(\"https://api.example.com\", System.getenv(\"API_KEY\")));\n{call}\n```\n\nThe package supports both Gradle (`build.gradle`) and Maven (`pom.xml`).\n\n{middleware}",
        api.name
    )
}

pub(super) fn style_guide(api: &Api, package: &str, style: SdkClientStyle) -> String {
    let surface = match style {
        SdkClientStyle::Flat => {
            "## Flat client\n\nOperations are exposed directly on `Client`, for example `client.getContact(input)`. This package was generated with `SdkClientStyle.Flat`."
        }
        SdkClientStyle::Namespaced => {
            "## Namespaced client\n\nOperations are grouped by the first OpenAPI tag, falling back to a stable path resource. Use `client.contacts().get(input)`. Direct methods remain on `Client` for migration. This package was generated with `SdkClientStyle.Namespaced`."
        }
    };
    format!(
        "# {} Java SDK style guide\n\nPackage: `{package}`.\n\n{surface}\n\n## Pagination\n\nA declared safe cursor or offset/limit contract adds `{{operation}}Pages(input)`, a lazy `Iterable` of the operation's normal response type. It reuses the ordinary operation for every page. Cursor inputs may be string query, header, or required path parameters; path cursors require an initial value under OpenAPI. Legacy offset/page inputs use optional direct integer query parameters; referenced integer control schemas do not receive helpers. declared `type: page` also accepts required integer query inputs and validates `outputs.results` against the response schema. Omitted pages default to 1 and offsets to 0; explicit zero is preserved. Results-based helpers stop on empty or short arrays, guard integer overflow, and reject negative inputs or nonpositive limits. Cursor and same-origin URL helpers stop on repeated continuations. Every helper stops after 10,000 pages. JSONPath field/array selectors and RFC 6901 pointers are supported. Body continuations stay explicit rather than being guessed.\n\n## Media and streaming\n\nNon-JSON success responses are returned as `byte[]`; non-JSON request bodies accept `byte[]`. A `text/event-stream` operation returns `Stream<String>` containing complete event data payloads (multiline data fields are joined with a newline; metadata and comments are ignored). Close that stream when finished.\n",
        api.name
    )
}
