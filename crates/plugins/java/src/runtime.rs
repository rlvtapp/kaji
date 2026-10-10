use super::*;

pub(super) fn api_exception(package: &str) -> String {
    format!(
        "package {package};\n\n\n{NOTICE}\n/** An unsuccessful HTTP response returned by the API. */\npublic class ApiException extends RuntimeException {{\n  private final int statusCode;\n  \n    private final String responseBody;\n  \n    private final String retryAfter;\n  \n    private final String retryAfterMillis;\n  \n\n    public ApiException(int statusCode, String responseBody) {{\n    this(statusCode, responseBody, null);\n\n  }}\n  \n\n    public ApiException(int statusCode, String responseBody, String retryAfter) {{\n    this(statusCode, responseBody, retryAfter, null);\n\n  }}\n  \n\n    public ApiException(int statusCode, String responseBody, String retryAfter, String retryAfterMillis) {{\n    super(\"Poolster API request failed with HTTP \" + statusCode);\n    \n        this.statusCode = statusCode;\n    \n        this.responseBody = responseBody;\n    \n        this.retryAfter = retryAfter;\n    \n        this.retryAfterMillis = retryAfterMillis;\n    \n\n  }}\n  \n\n    public int statusCode() {{\n    return statusCode;\n\n  }}\n  \n    public String responseBody() {{\n    return responseBody;\n\n  }}\n  \n    /** Raw Retry-After value, if the API sent one. */\n    public String retryAfter() {{\n    return retryAfter;\n\n  }}\n  \n    /** Raw retry-after-ms value, if present. */\n    public String retryAfterMillis() {{\n    return retryAfterMillis;\n\n  }}\n  \n}}\n\n"
    )
}

pub(super) fn client_config(package: &str) -> String {
    format!(
        "package {package};\n\n\nimport java.net.http.HttpClient;\n\nimport java.time.Duration;\n\nimport java.util.Map;\n\n\n{NOTICE}\n/** Immutable configuration for a generated API client. */\npublic record ClientConfig(\n        String baseUrl,\n        String apiKey,\n        String apiKeyHeader,\n        String apiKeyPrefix,\n        Map<String, String> defaultHeaders,\n        HttpClient httpClient,\n        Duration timeout,\n        RetryConfig retry,\n        ClientHooks hooks\n) {{\n  public ClientConfig(String baseUrl, String apiKey) {{\n    this(baseUrl, apiKey, \"Authorization\", \"Bearer\", Map.of(), null, Duration.ofSeconds(30), null, null);\n    \n\n  }}\n  \n\n    public ClientConfig {{\n    if (baseUrl == null || baseUrl.isBlank()) {{\n      throw new IllegalArgumentException(\"baseUrl is required\");\n      \n\n    }}\n    \n        apiKeyHeader = apiKeyHeader == null || apiKeyHeader.isBlank() ? \"Authorization\" : apiKeyHeader;\n    \n        apiKeyPrefix = apiKeyPrefix == null ? \"\" : apiKeyPrefix;\n    \n        defaultHeaders = defaultHeaders == null ? Map.of() : Map.copyOf(defaultHeaders);\n    \n        timeout = timeout == null ? Duration.ofSeconds(30) : timeout;\n    \n        retry = retry == null ? RetryConfig.defaults() : retry;\n    \n\n  }}\n  \n}}\n\n"
    )
}

pub(super) fn retry_config(package: &str) -> String {
    format!(
        "package {package};\n\n\nimport java.time.Duration;\n\n\n{NOTICE}\n/** Retry policy for requests Poolster can safely replay. */\npublic record RetryConfig(int maxAttempts, Duration initialDelay, Duration maxDelay) {{\n  public RetryConfig {{\n    if (maxAttempts < 1) throw new IllegalArgumentException(\"maxAttempts must be at least 1\");\n    \n        initialDelay = initialDelay == null ? Duration.ofMillis(250) : initialDelay;\n    \n        maxDelay = maxDelay == null ? Duration.ofSeconds(8) : maxDelay;\n    \n\n  }}\n  \n\n    public static RetryConfig defaults() {{\n    return new RetryConfig(3, Duration.ofMillis(250), Duration.ofSeconds(8));\n\n  }}\n  \n}}\n\n"
    )
}

pub(super) fn client_hooks(package: &str) -> String {
    format!(
        "package {package};\n\n\nimport java.net.URI;\n\n\n{NOTICE}\n/** Optional lifecycle callbacks for package-level telemetry and policy. */\npublic interface ClientHooks {{\n  default void beforeRequest(String method, URI uri) {{}}\n    default void afterResponse(int statusCode) {{}}\n    default void onError(RuntimeException error) {{}}\n}}\n\n"
    )
}

/// Shared HTTP runtime. Operation methods deliberately live in small
/// inheritance chunks so a large spec does not create a monolithic Client.
pub(super) fn render_client_base(api: &Api, package: &str) -> String {
    let mut output = format!(
        "package {package};\n\n\nimport com.fasterxml.jackson.core.JsonProcessingException;\n\nimport com.fasterxml.jackson.databind.JsonNode;\n\nimport com.fasterxml.jackson.databind.ObjectMapper;\n\nimport com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;\n\nimport java.io.IOException;\n\nimport java.net.URI;\n\nimport java.net.URLEncoder;\n\nimport java.net.http.HttpClient;\n\nimport java.net.http.HttpRequest;\n\nimport java.net.http.HttpResponse;\n\nimport java.nio.charset.StandardCharsets;\n\nimport java.time.Duration;\n\nimport java.util.ArrayList;\n\nimport java.util.List;\n\nimport java.util.Locale;\n\nimport java.util.Map;\n\nimport java.util.Objects;\n\nimport {package}.model.*;\n\n\n{NOTICE}\n/** Thread-safe API client. Reuse one instance for the lifetime of an application. */\npublic final class Client {{\n  private final String baseUrl;\n  \n    private final String apiKey;\n  \n    private final String apiKeyHeader;\n  \n    private final String apiKeyPrefix;\n  \n    private final Map<String, String> defaultHeaders;\n  \n    private final HttpClient httpClient;\n  \n    private final Duration timeout;\n  \n    private final RetryConfig retry;\n  \n    private final ClientHooks hooks;\n  \n    private final ObjectMapper mapper;\n  \n"
    );
    output = output.replacen(
        "public final class Client {",
        "public class ClientBase {",
        1,
    );
    output.push_str("\n    protected ClientBase(ClientConfig config) {\n        Objects.requireNonNull(config, \"config\");\n        this.baseUrl = stripTrailingSlash(config.baseUrl());\n        this.apiKey = config.apiKey();\n        this.apiKeyHeader = config.apiKeyHeader();\n        this.apiKeyPrefix = config.apiKeyPrefix();\n        this.defaultHeaders = config.defaultHeaders();\n        this.timeout = config.timeout();\n        this.retry = config.retry();\n        this.hooks = config.hooks();\n        this.httpClient = config.httpClient() != null ? config.httpClient() : HttpClient.newBuilder().connectTimeout(timeout).build();\n        this.mapper = new ObjectMapper().registerModule(new JavaTimeModule());\n");
    output.push_str("    }\n\n");
    output.push_str(
        "    private String request(String method, String path, List<QueryParameter> query, Map<String, String> headers, java.lang.Object body) {\n        var url = baseUrl + path + queryString(query);\n        var builder = HttpRequest.newBuilder(URI.create(url)).timeout(timeout).header(\"Accept\", \"application/json\");\n        defaultHeaders.forEach(builder::header);\n        headers.forEach(builder::header);\n        if (apiKey != null && !apiKey.isBlank() && defaultHeaders.keySet().stream().noneMatch(name -> name.equalsIgnoreCase(apiKeyHeader)) && headers.keySet().stream().noneMatch(name -> name.equalsIgnoreCase(apiKeyHeader))) {\n            var credential = apiKeyPrefix == null || apiKeyPrefix.isBlank() ? apiKey : apiKeyPrefix + \" \" + apiKey;\n            builder.header(apiKeyHeader, credential);\n        }\n        try {\n            if (body == null) {\n                builder.method(method, HttpRequest.BodyPublishers.noBody());\n            } else {\n                var requestMedia=headers.getOrDefault(\"Content-Type\",\"application/json\");\n                builder.setHeader(\"Content-Type\",requestMedia);\n                var serialized=java.util.Set.of(\"application/x-ndjson\",\"application/ndjson\",\"application/jsonl\",\"application/json-seq\").contains(requestMedia)?encodeSequentialJson(body,requestMedia):mapper.writeValueAsString(body);\n                builder.method(method,HttpRequest.BodyPublishers.ofString(serialized));\n            }\n            var response = httpClient.send(builder.build(), HttpResponse.BodyHandlers.ofString());\n            if (response.statusCode() < 200 || response.statusCode() >= 300) {\n                throw new ApiException(response.statusCode(), response.body());\n            }\n            return normalizeSequentialJson(response.body(), response.headers().firstValue(\"Content-Type\").orElse(\"\"));\n        } catch (JsonProcessingException error) {\n            throw new IllegalArgumentException(\"Poolster could not serialize the request body\", error);\n        } catch (IOException error) {\n            throw new IllegalStateException(\"Poolster could not execute the request\", error);\n        } catch (InterruptedException error) {\n            Thread.currentThread().interrupt();\n            throw new IllegalStateException(\"Poolster request was interrupted\", error);\n        }\n    }\n\n    private <T> T decode(String response, Class<T> type) {\n        try {\n            return mapper.readValue(response, type);\n        } catch (JsonProcessingException error) {\n            throw new IllegalStateException(\"Poolster could not decode the API response\", error);\n        }\n    }\n\n    private static String stripTrailingSlash(String value) {\n        return value.endsWith(\"/\") ? value.substring(0, value.length() - 1) : value;\n    }\n\n    private static String pathValue(java.lang.Object value) {\n        return URLEncoder.encode(String.valueOf(value), StandardCharsets.UTF_8).replace(\"+\", \"%20\");\n    }\n\n    private static String queryString(List<QueryParameter> query) {\n        var parts = new ArrayList<String>();\n        for (var parameter : query) {\n            if (parameter.value() == null) continue;\n            if (parameter.value() instanceof Iterable<?> values) {\n                for (var value : values) if (value != null) parts.add(encodeQuery(parameter.name(), value));\n            } else {\n                parts.add(encodeQuery(parameter.name(), parameter.value()));\n            }\n        }\n        return parts.isEmpty() ? \"\" : \"?\" + String.join(\"&\", parts);\n    }\n\n    private static String encodeQuery(String name, java.lang.Object value) {\n        return URLEncoder.encode(name, StandardCharsets.UTF_8) + \"=\" + URLEncoder.encode(String.valueOf(value), StandardCharsets.UTF_8);\n    }\n\n    private record QueryParameter(String name, java.lang.Object value) {}\n}\n",
    );
    output.truncate(output.len() - 2);
    output.push_str(
        r#"
    /** Follow a generated pagination link only on this client's API origin. */
    private String requestPaginationUrlWithRetry(String method, String continuation, Map<String, String> headers, java.lang.Object body) {
        var url = resolvePaginationUrl(continuation);
        var maxAttempts = retryAllowed(method, headers, null) ? retry.maxAttempts() : 1;
        RuntimeException lastError = null;
        for (var attempt = 0; attempt < maxAttempts; attempt++) {
            try {
                if (hooks != null) hooks.beforeRequest(method, URI.create(url));
                var builder = HttpRequest.newBuilder(URI.create(url)).timeout(timeout).header("Accept", "application/json");
                defaultHeaders.forEach(builder::header); headers.forEach(builder::header);
                if (apiKey != null && !apiKey.isBlank()) { var credential = apiKeyPrefix == null || apiKeyPrefix.isBlank() ? apiKey : apiKeyPrefix + " " + apiKey; builder.header(apiKeyHeader, credential); }
                if (body == null) builder.method(method, HttpRequest.BodyPublishers.noBody());
                else if (body instanceof byte[] bytes) { builder.header("Content-Type", "application/octet-stream"); builder.method(method, HttpRequest.BodyPublishers.ofByteArray(bytes)); }
                else { builder.header("Content-Type", "application/json"); builder.method(method, HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body))); }
                var response = httpClient.send(builder.build(), HttpResponse.BodyHandlers.ofString());
                if (response.statusCode() < 200 || response.statusCode() >= 300) throw new ApiException(response.statusCode(), response.body(), response.headers().firstValue("Retry-After").orElse(null), response.headers().firstValue("retry-after-ms").orElse(null));
                if (hooks != null) hooks.afterResponse(response.statusCode());
                return response.body();
            } catch (JsonProcessingException error) { throw new IllegalArgumentException("Poolster could not serialize the request body", error);
            } catch (ApiException error) {
                lastError = error;
                if (attempt + 1 >= maxAttempts || !retryableStatus(error.statusCode())) { if (hooks != null) hooks.onError(error); throw error; }
                retryDelay(attempt, error.retryAfter(), error.retryAfterMillis());
            } catch (IOException error) {
                lastError = new IllegalStateException("Poolster could not execute the request", error);
                if (attempt + 1 >= maxAttempts) { if (hooks != null) hooks.onError(lastError); throw lastError; }
                retryDelay(attempt, null, null);
            } catch (InterruptedException error) { Thread.currentThread().interrupt(); throw new IllegalStateException("Poolster request was interrupted", error); }
        }
        if (hooks != null) hooks.onError(lastError);
        throw lastError == null ? new IllegalStateException("Poolster retry loop completed without a response") : lastError;
    }

    private String resolvePaginationUrl(String continuation) {
        var base = URI.create(baseUrl);
        var next = base.resolve(URI.create(continuation)).normalize();
        if (!Objects.equals(base.getScheme(), next.getScheme()) || !Objects.equals(base.getHost(), next.getHost()) || effectivePort(base) != effectivePort(next)) {
            throw new IllegalArgumentException("Poolster pagination URL must remain on the configured API origin");
        }
        return next.toString();
    }

    private static int effectivePort(URI uri) {
        if (uri.getPort() >= 0) return uri.getPort();
        return "https".equalsIgnoreCase(uri.getScheme()) ? 443 : 80;
    }
"#,
    );
    output = output.replacen(
        "throw new ApiException(response.statusCode(), response.body());",
        "throw new ApiException(response.statusCode(), response.body(), response.headers().firstValue(\"Retry-After\").orElse(null), response.headers().firstValue(\"retry-after-ms\").orElse(null));",
        1,
    );
    output = output.replacen(
        "            return response.body();",
        "            if (hooks != null) hooks.afterResponse(response.statusCode());\n            return response.body();",
        1,
    );
    // Add safe retries without making operation signatures expose a transport.
    // `request` builds a fresh JDK request each time, so replay never reuses a
    // single-use body publisher.
    if api
        .operations
        .iter()
        .any(|operation| java_pagination(operation).is_some())
    {
        output.push_str(
            r#"
    /** Conservative JSONPath evaluator for declared pagination outputs. */
    private static JsonNode poolsterJsonPath(JsonNode value, String path) {
        if (value == null || path == null) return null;
        if (path.startsWith("/")) {
            var current = value;
            for (var token : path.substring(1).split("/", -1)) {
                var key = token.replace("~1", "/").replace("~0", "~");
                if (current.isObject()) current = current.get(key);
                else if (current.isArray() && key.matches("0|[1-9][0-9]*")) {
                    try { var index = Integer.parseInt(key); current = index < current.size() ? current.get(index) : null; }
                    catch (NumberFormatException error) { return null; }
                } else return null;
                if (current == null) return null;
            }
            return current;
        }
        if (!path.startsWith("$")) return null;
        var current = value;
        int position = 1;
        while (position < path.length()) {
            if (path.charAt(position) == '.') {
                int start = ++position;
                while (position < path.length() && path.charAt(position) != '.' && path.charAt(position) != '[') position++;
                if (position == start || !current.isObject()) return null;
                current = current.get(path.substring(start, position));
            } else if (path.charAt(position) == '[') {
                int end = path.indexOf(']', position);
                if (end < 0 || !current.isArray()) return null;
                try {
                    long index = Long.parseLong(path.substring(position + 1, end));
                    if (index < 0) index = current.size() + index;
                    if (index < 0 || index >= current.size()) return null;
                    current = current.get((int) index);
                } catch (NumberFormatException error) { return null; }
                position = end + 1;
            } else return null;
            if (current == null) return null;
        }
        return current;
    }
"#,
        );
    }
    output.push_str(
        "\n    /** Downloads a non-JSON representation as bytes. */\n    private byte[] requestBinary(String method, String path, List<QueryParameter> query, Map<String, String> headers, java.lang.Object body) {\n        var url = baseUrl + path + queryString(query);\n        var builder = HttpRequest.newBuilder(URI.create(url)).timeout(timeout).header(\"Accept\", \"*/*\");\n        defaultHeaders.forEach(builder::header); headers.forEach(builder::header);\n        if (apiKey != null && !apiKey.isBlank()) { var credential = apiKeyPrefix == null || apiKeyPrefix.isBlank() ? apiKey : apiKeyPrefix + \" \" + apiKey; builder.header(apiKeyHeader, credential); }\n        try {\n            if (body == null) builder.method(method, HttpRequest.BodyPublishers.noBody());\n            else if (body instanceof byte[] bytes) { builder.header(\"Content-Type\", \"application/octet-stream\"); builder.method(method, HttpRequest.BodyPublishers.ofByteArray(bytes)); }\n            else { builder.header(\"Content-Type\", \"application/json\"); builder.method(method, HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body))); }\n            var response = httpClient.send(builder.build(), HttpResponse.BodyHandlers.ofByteArray());\n            if (response.statusCode() < 200 || response.statusCode() >= 300) throw new ApiException(response.statusCode(), new String(response.body(), StandardCharsets.UTF_8), response.headers().firstValue(\"Retry-After\").orElse(null), response.headers().firstValue(\"retry-after-ms\").orElse(null));\n            if (hooks != null) hooks.afterResponse(response.statusCode());\n            return response.body();\n        } catch (JsonProcessingException error) { throw new IllegalArgumentException(\"Poolster could not serialize the request body\", error);\n        } catch (IOException error) { throw new IllegalStateException(\"Poolster could not execute the request\", error);\n        } catch (InterruptedException error) { Thread.currentThread().interrupt(); throw new IllegalStateException(\"Poolster request was interrupted\", error); }\n    }\n\n    /** Returns data lines from a text/event-stream endpoint. Close the stream when finished. */\n    private java.util.stream.Stream<String> requestEventStream(String method, String path, List<QueryParameter> query, Map<String, String> headers, java.lang.Object body) {\n        var url = baseUrl + path + queryString(query);\n        var builder = HttpRequest.newBuilder(URI.create(url)).timeout(timeout).header(\"Accept\", \"text/event-stream\");\n        defaultHeaders.forEach(builder::header); headers.forEach(builder::header);\n        if (apiKey != null && !apiKey.isBlank()) { var credential = apiKeyPrefix == null || apiKeyPrefix.isBlank() ? apiKey : apiKeyPrefix + \" \" + apiKey; builder.header(apiKeyHeader, credential); }\n        try {\n            if (body == null) builder.method(method, HttpRequest.BodyPublishers.noBody());\n            else if (body instanceof byte[] bytes) { builder.header(\"Content-Type\", \"application/octet-stream\"); builder.method(method, HttpRequest.BodyPublishers.ofByteArray(bytes)); }\n            else { builder.header(\"Content-Type\", \"application/json\"); builder.method(method, HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body))); }\n            var response = httpClient.send(builder.build(), HttpResponse.BodyHandlers.ofLines());\n            if (response.statusCode() < 200 || response.statusCode() >= 300) { try (var lines = response.body()) { throw new ApiException(response.statusCode(), lines.reduce(\"\", (a, b) -> a + \"\\n\" + b), response.headers().firstValue(\"Retry-After\").orElse(null), response.headers().firstValue(\"retry-after-ms\").orElse(null)); } }\n            if (hooks != null) hooks.afterResponse(response.statusCode());\n            return ssePayloads(response.body());\n        } catch (JsonProcessingException error) { throw new IllegalArgumentException(\"Poolster could not serialize the request body\", error);\n        } catch (IOException error) { throw new IllegalStateException(\"Poolster could not execute the event stream\", error);\n        } catch (InterruptedException error) { Thread.currentThread().interrupt(); throw new IllegalStateException(\"Poolster request was interrupted\", error); }\n    }\n"
    );
    output.push_str(
        "\n    private String requestWithRetry(String method, String path, List<QueryParameter> query, Map<String, String> headers, java.lang.Object body, String idempotencyHeader) {\n        var maxAttempts = retryAllowed(method, headers, idempotencyHeader) ? retry.maxAttempts() : 1;\n        RuntimeException lastError = null;\n        for (var attempt = 0; attempt < maxAttempts; attempt++) {\n            try {\n                if (hooks != null) hooks.beforeRequest(method, URI.create(baseUrl + path + queryString(query)));\n                var response = request(method, path, query, headers, body);\n                if (hooks != null) hooks.afterResponse(200);\n                return response;\n            } catch (ApiException error) {\n                lastError = error;\n                if (attempt + 1 >= maxAttempts || !retryableStatus(error.statusCode())) {\n                    if (hooks != null) hooks.onError(error);\n                    throw error;\n                }\n                retryDelay(attempt, error.retryAfter(), error.retryAfterMillis());\n            } catch (IllegalStateException error) {\n                lastError = error;\n                if (attempt + 1 >= maxAttempts || !(error.getCause() instanceof IOException)) {\n                    if (hooks != null) hooks.onError(error);\n                    throw error;\n                }\n                retryDelay(attempt, null, null);\n            }\n        }\n        if (hooks != null) hooks.onError(lastError);\n        throw lastError == null ? new IllegalStateException(\"Poolster retry loop completed without a response\") : lastError;\n    }\n\n    private static boolean retryAllowed(String method, Map<String, String> headers, String idempotencyHeader) {\n        var normalized = method.toUpperCase(Locale.ROOT);\n        return (idempotencyHeader != null && headers.entrySet().stream().anyMatch(entry -> entry.getKey().equalsIgnoreCase(idempotencyHeader) && entry.getValue() != null && !entry.getValue().isBlank())) || normalized.equals(\"GET\") || normalized.equals(\"HEAD\") || normalized.equals(\"OPTIONS\") || normalized.equals(\"TRACE\") || normalized.equals(\"QUERY\") || normalized.equals(\"PUT\") || normalized.equals(\"DELETE\")\n            || ((normalized.equals(\"POST\") || normalized.equals(\"PATCH\")) && headers.entrySet().stream().anyMatch(entry -> entry.getKey().equalsIgnoreCase(\"Idempotency-Key\") && entry.getValue() != null && !entry.getValue().isBlank()));\n    }\n\n    private static boolean retryableStatus(int status) {\n        return status == 408 || status == 429 || status == 500 || status == 502 || status == 503 || status == 504;\n    }\n\n    private void retryDelay(int attempt, String retryAfter, String retryAfterMillis) {\n        long delay = retryAfterMillisDelay(retryAfterMillis);\n        if (delay < 0) delay = retryAfterDelay(retryAfter);\n        var cap = boundedDurationMillis(retry.maxDelay());\n        if (delay < 0) {\n            var exponential = boundedDurationMillis(retry.initialDelay()) * Math.pow(2, Math.min(attempt, 30));\n            delay = (long) Math.min(exponential, cap);\n        }\n        delay = Math.min(delay, cap);\n        try {\n            if (delay > 0) Thread.sleep(delay);\n        } catch (InterruptedException error) {\n            Thread.currentThread().interrupt();\n            throw new IllegalStateException(\"Poolster retry was interrupted\", error);\n        }\n    }\n\n    private static long boundedDurationMillis(java.time.Duration duration) {\n        if (duration.isNegative()) return 0;\n        try { return duration.toMillis(); } catch (ArithmeticException overflow) { return Long.MAX_VALUE; }\n    }\n\n    private static long retryAfterMillisDelay(String value) {\n        if (value == null || value.isBlank()) return -1;\n        try {\n            var milliseconds = Double.parseDouble(value.trim());\n            if (!Double.isFinite(milliseconds) || milliseconds < 0) return -1;\n            return milliseconds >= Long.MAX_VALUE ? Long.MAX_VALUE : (long) milliseconds;\n        } catch (NumberFormatException ignored) { return -1; }\n    }\n\n    private static long retryAfterDelay(String value) {\n        if (value == null || value.isBlank()) return -1;\n        try {\n            var seconds = Double.parseDouble(value.trim());\n            if (!Double.isFinite(seconds) || seconds < 0) return -1;\n            return seconds >= Long.MAX_VALUE / 1000.0 ? Long.MAX_VALUE : (long) (seconds * 1000);\n        } catch (NumberFormatException ignored) {\n            try {\n                var date = java.time.ZonedDateTime.parse(value.trim(), java.time.format.DateTimeFormatter.RFC_1123_DATE_TIME).toInstant();\n                var duration = java.time.Duration.between(java.time.Instant.now(), date);\n                if (duration.isNegative()) return 0;\n                try { return duration.toMillis(); } catch (ArithmeticException overflow) { return Long.MAX_VALUE; }\n            } catch (java.time.DateTimeException malformed) { return -1; }\n        }\n    }\n}\n",
    );
    output = output.replacen(
        "                var response = request(method, path, query, headers, body);\n                if (hooks != null) hooks.afterResponse(200);\n                return response;",
        "                var response = request(method, path, query, headers, body);\n                return response;",
        1,
    );
    // Operation chunk subclasses need the transport helpers and state, but
    // they stay package-internal to generated SDK consumers.
    output.truncate(output.len() - 2);
    output.push_str(sse_parser());
    output.push_str(include_str!("../templates/sequential_json.java.tmpl"));
    output.push_str(include_str!("../templates/call_options_methods.java.tmpl"));
    output.push_str("}\n");
    let output = output.replace("    private ", "    protected ").replace(
        "protected record QueryParameter",
        "public record QueryParameter",
    );
    if api.operations.iter().any(multipart::selected) {
        multipart::runtime(output)
    } else {
        output
    }
}

pub(super) fn sse_parser() -> &'static str {
    r#"
    /** Lazy SSE payload framing. Metadata/comment fields are not data. */
    private static java.util.stream.Stream<String> ssePayloads(java.util.stream.Stream<String> lines) {
        var iterator = lines.iterator();
        var splitter = new java.util.Spliterators.AbstractSpliterator<String>(Long.MAX_VALUE, java.util.Spliterator.ORDERED | java.util.Spliterator.NONNULL) {
            @Override public boolean tryAdvance(java.util.function.Consumer<? super String> action) {
                var data = new java.util.ArrayList<String>();
                while (iterator.hasNext()) {
                    var line = iterator.next();
                    if (line.isEmpty()) {
                        if (!data.isEmpty()) { action.accept(String.join("\n", data)); return true; }
                    } else if (line.equals("data")) { data.add(""); }
                    else if (line.startsWith("data:")) {
                        var value = line.substring(5);
                        data.add(value.startsWith(" ") ? value.substring(1) : value);
                    }
                }
                if (!data.isEmpty()) { action.accept(String.join("\n", data)); return true; }
                return false;
            }
        };
        return java.util.stream.StreamSupport.stream(splitter, false).onClose(lines::close);
    }
"#
}
