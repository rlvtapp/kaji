use super::*;

pub(super) fn render_client(api: &Api, namespace: &str, style: SdkClientStyle) -> String {
    let mut output = format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace};\n\nuse {namespace}\\Exceptions\\ApiException;\nuse Nyholm\\Psr7\\Factory\\Psr17Factory;\nuse Psr\\Http\\Client\\ClientInterface;\nuse Psr\\Http\\Message\\RequestFactoryInterface;\nuse Psr\\Http\\Message\\StreamFactoryInterface;\nuse Psr\\Http\\Message\\StreamInterface;\n"
    );
    output.push_str(&format!("\n{NOTICE}\nfinal class Client\n{{\n    private readonly RequestFactoryInterface $requestFactory;\n    private readonly StreamFactoryInterface $streamFactory;\n"));
    for part in 0..php_operation_groups(api, namespace, &NamedTypes::from_api(api)).len() {
        let _ = writeln!(output, "    use ClientOperations{part:03};");
    }
    if style == SdkClientStyle::Namespaced {
        for (resource, _) in resource_operations(api) {
            let property = property_name(&resource);
            let _ = writeln!(
                output,
                "    private ?\\{namespace}\\Resources\\{resource}Resource ${property}Resource = null;"
            );
        }
    }
    if style == SdkClientStyle::Namespaced {
        output.push_str("    public function __clone(): void\n    {\n");
        for (resource, _) in resource_operations(api) {
            let property = property_name(&resource);
            let _ = writeln!(output, "        $this->{property}Resource = null;");
        }
        output.push_str("    }\n");
    }
    output.push_str("\n    /**\n     * @param array<string, string> $defaultHeaders\n     * @param null|callable(array<string, mixed>): void $beforeRequest\n     * @param null|callable(array<string, mixed>): void $afterResponse\n     * @param null|callable(\\Throwable, array<string, mixed>): void $onError\n     */\n    public function __construct(\n        private readonly ClientInterface $httpClient,\n        string $baseUrl,\n        private readonly ?string $apiKey = null,\n        private readonly string $apiKeyHeader = 'Authorization',\n        private readonly string $apiKeyPrefix = 'Bearer',\n        private array $defaultHeaders = [],\n        ?RequestFactoryInterface $requestFactory = null,\n        ?StreamFactoryInterface $streamFactory = null,\n        private readonly int $maxRetries = 2,\n        private readonly int $retryInitialDelayMs = 250,\n        private readonly int $retryMaxDelayMs = 8000,\n        private readonly mixed $beforeRequest = null,\n        private readonly mixed $afterResponse = null,\n        private readonly mixed $onError = null,\n    ) {\n        $this->baseUrl = rtrim($baseUrl, '/');\n        $factory = new Psr17Factory();\n        $this->requestFactory = $requestFactory ?? $factory;\n        $this->streamFactory = $streamFactory ?? $factory;\n    }\n\n    /** Independent headers and optional already-configured PSR-18 driver for one call. */\n    public function forCall(array $headers = [], ?ClientInterface $httpClient = null): self\n    {\n        $scoped = clone $this;\n        foreach ($headers as $name => $value) {\n            if (!is_string($name) || $name === '' || !is_string($value) || strpbrk($name . $value, \"\\r\\n\") !== false) { throw new \\InvalidArgumentException('Invalid call headers'); }\n            foreach (array_keys($scoped->defaultHeaders) as $existing) { if (strcasecmp($existing, $name) === 0) { unset($scoped->defaultHeaders[$existing]); } }\n            $scoped->defaultHeaders[$name] = $value;\n        }\n        if ($httpClient !== null) { $scoped->callHttpClient = $httpClient; }\n        return $scoped;\n    }\n\n    private ?ClientInterface $callHttpClient = null;\n    private readonly string $baseUrl;\n\n");
    if style == SdkClientStyle::Namespaced {
        for (resource, _) in resource_operations(api) {
            let accessor = resource_accessor_name(api, &resource);
            let property = property_name(&resource);
            let _ = writeln!(
                output,
                "    public function {accessor}(): \\{namespace}\\Resources\\{resource}Resource\n    {{\n        return $this->{property}Resource ??= new \\{namespace}\\Resources\\{resource}Resource($this);\n    }}\n"
            );
        }
    }
    if api.operations.iter().any(|operation| {
        page_pagination::plan(api, operation)
            .ok()
            .flatten()
            .is_some()
            || cursor_pagination(api, operation).is_some()
            || offset_pagination(operation).is_some()
            || url_pagination(operation).is_some()
    }) {
        output.push_str(render_pagination_helper());
    }
    output.push_str("    /** @return array<string, string> */\n    private function authHeaders(): array\n    {\n        if ($this->apiKey === null || $this->apiKey === '') {\n            return [];\n        }\n\n        $value = trim($this->apiKeyPrefix . ' ' . $this->apiKey);\n        return [$this->apiKeyHeader => $value];\n    }\n\n    /** @param array<string, string> $headers */\n    private function request(string $method, string $path, array $query, array $headers, mixed $body, string $bodyKind = 'json', bool $retryable = false): string\n    {\n        $url = $this->baseUrl . $path;\n        $query = array_filter($query, static fn (mixed $value): bool => $value !== null);\n        if ($query !== []) {\n            $url .= '?' . http_build_query($query, '', '&', PHP_QUERY_RFC3986);\n        }\n        $requestHeaders = array_filter(array_merge($this->defaultHeaders, $this->authHeaders(), $headers), static fn(mixed $value): bool => $value !== null);\n        $context = ['method' => $method, 'url' => $url, 'query' => $query, 'headers' => $requestHeaders, 'body' => $body];\n        if ($this->beforeRequest !== null) {\n            ($this->beforeRequest)($context);\n        }\n        $canRetry = $retryable && $this->retryAllowed($method, $requestHeaders, $idempotencyHeader);\n        for ($attempt = 0; $attempt <= max(0, $this->maxRetries); $attempt++) {\n            $request = $this->requestFactory->createRequest($method, $url);\n            foreach ($requestHeaders as $name => $value) {\n                $request = $request->withHeader($name, $value);\n            }\n            if ($body !== null) {\n                if ($bodyKind === 'binary') {\n                    if (!is_string($body)) {\n                        throw new \\TypeError('binary request bodies must be strings');\n                    }\n                    $encoded = $body;\n                    $contentType = 'application/octet-stream';\n                } elseif ($bodyKind === 'form') {\n                    if (!is_array($body)) {\n                        throw new \\TypeError('form request bodies must be arrays');\n                    }\n                    $encoded = http_build_query($body, '', '&', PHP_QUERY_RFC3986);\n                    $contentType = 'application/x-www-form-urlencoded';\n                } else {\n                    $encoded = json_encode($body, JSON_THROW_ON_ERROR);\n                    $contentType = 'application/json';\n                }\n                $request = $request\n                    ->withHeader('Content-Type', $contentType)\n                    ->withBody($this->streamFactory->createStream($encoded));\n            }\n            try {\n                $response = ($this->callHttpClient ?? $this->httpClient)->sendRequest($request);\n            } catch (\\Throwable $error) {\n                if ($canRetry && $attempt < max(0, $this->maxRetries)) {\n                    $this->retryDelay($attempt);\n                    continue;\n                }\n                $this->notifyError($error, $context);\n                throw $error;\n            }\n            $status = $response->getStatusCode();\n            if ($canRetry && $this->retryableStatus($status) && $attempt < max(0, $this->maxRetries)) {\n                $this->retryDelay($attempt, $response->getHeaderLine('Retry-After'), $response->getHeaderLine('retry-after-ms'));\n                continue;\n            }\n            $contents = (string) $response->getBody();\n            if ($status < 200 || $status >= 300) {\n                $error = new ApiException(sprintf('API request failed with HTTP %d', $status), $status, $contents);\n                $this->notifyError($error, $context);\n                throw $error;\n            }\n            if ($this->afterResponse !== null) {\n                ($this->afterResponse)(['request' => $context, 'statusCode' => $status, 'headers' => $response->getHeaders(), 'body' => $contents]);\n            }\n            return $contents;\n        }\n        throw new \\LogicException('Poolster retry loop completed without a response');\n    }\n\n    /** @param array<string, string> $headers */\n    private function retryAllowed(string $method, array $headers, ?string $idempotencyHeader = null): bool\n    {\n        if (in_array(strtoupper($method), ['GET', 'HEAD', 'OPTIONS', 'TRACE', 'QUERY', 'PUT', 'DELETE'], true)) {\n            return true;\n        }\n        if (!in_array(strtoupper($method), ['POST', 'PATCH'], true)) {\n            return false;\n        }\n        foreach ($headers as $name => $value) {\n            if ((strtolower($name) === 'idempotency-key' || ($idempotencyHeader !== null && strcasecmp($name, $idempotencyHeader) === 0)) && trim((string) $value) !== '') {\n                return true;\n            }\n        }\n        return false;\n    }\n\n    private function retryableStatus(int $status): bool\n    {\n        return in_array($status, [408, 429, 500, 502, 503, 504], true);\n    }\n\n    private function retryDelay(int $attempt, string $retryAfter = '', string $retryAfterMs = ''): void\n    {\n        $serverDelay = null;\n        foreach ([[$retryAfterMs, 1], [$retryAfter, 1000]] as [$value, $multiplier]) {\n            if (is_numeric($value) && is_finite((float) $value) && (float) $value >= 0) {\n                $serverDelay = (float) $value * $multiplier;\n                break;\n            }\n        }\n        $exponential = max(0, $this->retryInitialDelayMs) * (2 ** $attempt);\n        $milliseconds = min($serverDelay ?? $exponential, max(0, $this->retryMaxDelayMs));\n        if ($milliseconds > 0) {\n            usleep((int) round($milliseconds * 1000));\n        }\n    }\n\n    /** @param array<string, mixed> $context */\n    private function notifyError(\\Throwable $error, array $context): void\n    {\n        if ($this->onError !== null) {\n            ($this->onError)($error, $context);\n        }\n    }\n}\n");
    output = output.replace("    private function authHeaders(): array", "    private static function poolsterIdempotencyKey(): string\n    {\n        $bytes = random_bytes(16);\n        $bytes[6] = chr((ord($bytes[6]) & 15) | 64);\n        $bytes[8] = chr((ord($bytes[8]) & 63) | 128);\n        $hex = bin2hex($bytes);\n        return substr($hex, 0, 8) . '-' . substr($hex, 8, 4) . '-' . substr($hex, 12, 4) . '-' . substr($hex, 16, 4) . '-' . substr($hex, 20);\n    }\n\n    private function authHeaders(): array");
    // Keep the ordinary generated operation as the sole place that owns
    // auth, request serialization, retries, and hooks. A URL paginator only
    // replaces the URL at this private transport boundary.
    output = output.replacen(
        "private function request(string $method, string $path, array $query, array $headers, mixed $body, string $bodyKind = 'json', bool $retryable = false): string\n    {\n        $url = $this->baseUrl . $path;\n        $query = array_filter($query, static fn (mixed $value): bool => $value !== null);\n        if ($query !== []) {\n            $url .= '?' . http_build_query($query, '', '&', PHP_QUERY_RFC3986);\n        }",
        "private function request(string $method, string $path, array $query, array $headers, mixed $body, string $bodyKind = 'json', bool $retryable = false, ?string $paginationUrl = null, ?string $idempotencyHeader = null): string\n    {\n        $url = $this->baseUrl . $path;\n        $query = array_filter($query, static fn (mixed $value): bool => $value !== null);\n        if ($query !== []) {\n            $url .= '?' . http_build_query($query, '', '&', PHP_QUERY_RFC3986);\n        }\n        if ($paginationUrl !== null) {\n            $url = $this->resolvePaginationUrl($paginationUrl);\n        }",
        1,
    );
    output = output.replacen(
        "    /** @param array<string, string> $headers */\n    private function retryAllowed",
        "    /** Continue only on the configured API origin; never follow arbitrary links. */\n    private function resolvePaginationUrl(string $nextUrl): string\n    {\n        $base = parse_url($this->baseUrl);\n        if ($base === false || !isset($base['scheme'], $base['host'])) {\n            throw new \\LogicException('baseUrl must be absolute for URL pagination');\n        }\n        $candidate = parse_url($nextUrl);\n        if ($candidate === false) {\n            throw new \\InvalidArgumentException('pagination URL is invalid');\n        }\n        $baseScheme = strtolower($base['scheme']);\n        $baseHost = strtolower($base['host']);\n        $basePort = $base['port'] ?? ($baseScheme === 'https' ? 443 : 80);\n        if (isset($candidate['scheme']) || isset($candidate['host'])) {\n            $scheme = strtolower((string) ($candidate['scheme'] ?? ''));\n            $host = strtolower((string) ($candidate['host'] ?? ''));\n            $port = $candidate['port'] ?? ($scheme === 'https' ? 443 : 80);\n            if ($scheme !== $baseScheme || $host !== $baseHost || $port !== $basePort) {\n                throw new \\InvalidArgumentException('pagination URL must remain on the configured API origin');\n            }\n            return $nextUrl;\n        }\n        if (!str_starts_with($nextUrl, '/') || str_starts_with($nextUrl, '//')) {\n            throw new \\InvalidArgumentException('relative pagination URL must be root-relative');\n        }\n        $authority = $baseScheme . '://' . $baseHost;\n        if (isset($base['port'])) {\n            $authority .= ':' . $base['port'];\n        }\n        return $authority . $nextUrl;\n    }\n\n    /** @param array<string, string> $headers */\n    private function retryAllowed",
        1,
    );
    let sse_runtime = r#"
    /**
     * Opens a declared Server-Sent Events response without buffering or
     * decoding it. PSR-18 only promises a PSR-7 response, not a live socket;
     * choose a stream-capable implementation when live delivery is required.
     * Poolster intentionally does not reconnect a stream after any event.
     *
     * @param array<string, string> $headers
     */
    private function eventStream(string $method, string $path, array $query, array $headers, mixed $body, string $bodyKind = 'json'): StreamInterface
    {
        $url = $this->baseUrl . $path;
        $query = array_filter($query, static fn (mixed $value): bool => $value !== null);
        if ($query !== []) {
            $url .= '?' . http_build_query($query, '', '&', PHP_QUERY_RFC3986);
        }
        $requestHeaders = array_merge($this->defaultHeaders, $this->authHeaders(), ['Accept' => 'text/event-stream'], $headers);
        $context = ['method' => $method, 'url' => $url, 'query' => $query, 'headers' => $requestHeaders, 'body' => $body];
        if ($this->beforeRequest !== null) {
            ($this->beforeRequest)($context);
        }
        $request = $this->requestFactory->createRequest($method, $url);
        foreach ($requestHeaders as $name => $value) {
            $request = $request->withHeader($name, $value);
        }
        if ($body !== null) {
            if ($bodyKind === 'binary') {
                if (!is_string($body)) {
                    throw new \TypeError('binary request bodies must be strings');
                }
                $encoded = $body;
                $contentType = 'application/octet-stream';
            } elseif ($bodyKind === 'form') {
                if (!is_array($body)) {
                    throw new \TypeError('form request bodies must be arrays');
                }
                $encoded = http_build_query($body, '', '&', PHP_QUERY_RFC3986);
                $contentType = 'application/x-www-form-urlencoded';
            } else {
                $encoded = json_encode($body, JSON_THROW_ON_ERROR);
                $contentType = 'application/json';
            }
            $request = $request
                ->withHeader('Content-Type', $contentType)
                ->withBody($this->streamFactory->createStream($encoded));
        }
        try {
            $response = ($this->callHttpClient ?? $this->httpClient)->sendRequest($request);
        } catch (\Throwable $error) {
            $this->notifyError($error, $context);
            throw $error;
        }
        $status = $response->getStatusCode();
        if ($status < 200 || $status >= 300) {
            $contents = (string) $response->getBody();
            $error = new ApiException(sprintf('API request failed with HTTP %d', $status), $status, $contents);
            $this->notifyError($error, $context);
            throw $error;
        }
        if ($this->afterResponse !== null) {
            ($this->afterResponse)(['request' => $context, 'statusCode' => $status, 'headers' => $response->getHeaders(), 'body' => null]);
        }
        return $response->getBody();
    }
"#;
    let close = output
        .rfind("\n}\n")
        .expect("generated PHP client has a class closing brace");
    output.insert_str(close, sse_runtime);
    output = output.replace(
        "http_build_query($query, '', '&', PHP_QUERY_RFC3986)",
        "$this->poolsterQueryString($query)",
    );
    let query_runtime = r#"
    private function poolsterParameterContent(mixed $value, string $contentType): string
    {
        if ($contentType === 'application/json' || str_ends_with($contentType, '+json')) return json_encode($value, JSON_THROW_ON_ERROR);
        if ($contentType === 'application/x-www-form-urlencoded' && is_array($value)) return $this->poolsterQueryString($value);
        if (!is_string($value)) throw new \InvalidArgumentException('parameter content must be a serialized string');
        return $value;
    }

    private function poolsterWholeQuery(mixed $value, string $contentType, bool $allowNull = false): string
    {
        if ($value === null && !$allowNull) return '';
        if ($contentType === 'application/json' || str_ends_with($contentType, '+json')) return rawurlencode(json_encode($value, JSON_THROW_ON_ERROR));
        if (is_array($value) && $contentType === 'application/x-www-form-urlencoded') return $this->poolsterQueryString($value);
        if (!is_string($value)) throw new \InvalidArgumentException('whole query must be a serialized string');
        if (str_starts_with($value, '?')) $value = substr($value, 1);
        if (strpbrk($value, "?#\r\n") !== false) throw new \InvalidArgumentException('unsafe whole query');
        return $value;
    }

    private function poolsterSequentialJson(string $body, string $contentType): array
    {
        if (strlen($body) > 64 * 1024 * 1024) throw new \UnexpectedValueException('sequential response exceeds 64 MiB');
        if ($contentType === 'application/json-seq') {
            $records = explode("\x1e", $body);
            $prefix = array_shift($records);
            if (trim($prefix) !== '' || (trim($body) !== '' && $records === [])) throw new \UnexpectedValueException('JSON sequence requires record separators');
        } else {
            $records = array_values(array_filter(explode("\n", $body), static fn(string $line): bool => trim($line) !== ''));
        }
        return array_map(static fn(string $record): mixed => json_decode($record, true, 512, JSON_THROW_ON_ERROR), $records);
    }

    /** OpenAPI form/explode query scalars and repeated list values. */
    private function poolsterQueryString(array|string $query): string
    {
        if (is_string($query)) return $query;
        $pairs = [];
        foreach ($query as $name => $value) {
            foreach (is_array($value) ? $value : [$value] as $item) {
                if ($item === null) continue;
                $scalar = is_bool($item) ? ($item ? 'true' : 'false') : (string)$item;
                $pairs[] = rawurlencode((string)$name) . '=' . rawurlencode($scalar);
            }
        }
        return implode('&', $pairs);
    }
"#;
    let close = output
        .rfind("\n}\n")
        .expect("PHP client class closing brace");
    output.insert_str(close, query_runtime);
    let body_branch = "if ($bodyKind === 'binary') {";
    let multipart_branch = format!(
        r"if ($body instanceof \{namespace}\MultipartBody) {{
                    if (!in_array($bodyKind, ['multipart', 'multipart-json'], true)) throw new \InvalidArgumentException('operation does not accept multipart/form-data');
                    [$contentType, $encoded] = $body->encode();
                }} elseif ($bodyKind === 'multipart') {{
                    throw new \InvalidArgumentException('multipart request body must be MultipartBody');
                }} elseif ($bodyKind === 'binary') {{"
    );
    output = output.replace(body_branch, &multipart_branch);
    output = output.replace(
        "array $query, array $headers",
        "array|string $query, array $headers",
    );
    output = output.replace("$query = array_filter($query, static fn (mixed $value): bool => $value !== null);", "$query = is_array($query) ? array_filter($query, static fn (mixed $value): bool => $value !== null) : $query;");
    output = output.replace("if ($query !== [])", "if ($query !== [] && $query !== '')");
    output
}
