//! Generated synchronous client implementation.
use super::super::Api;

pub(super) fn render_runtime_client(api: &Api) -> String {
    format!(
        r#"class BaseClient:
    """Configurable synchronous client for {}."""

    def __init__(
        self,
        base_url: str,
        *,
        api_key: str | None = None,
        bearer_token: str | None = None,
        token_provider: Callable[[str | None], str] | None = None,
        headers: dict[str, str] | None = None,
        timeout: float = 30.0,
        max_retries: int = 2,
        retry_initial_delay: float = 0.25,
        retry_max_delay: float = 8.0,
        middleware: tuple[Callable[..., Any], ...] = (),
        validate_responses: bool = False,
        before_request: Callable[[dict[str, Any]], None] | None = None,
        after_response: Callable[[dict[str, Any]], None] | None = None,
        on_error: Callable[[Exception, dict[str, Any]], None] | None = None,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.bearer_token = bearer_token
        self.token_provider = token_provider
        self.headers = dict(headers or {{}})
        self.timeout = timeout
        self.max_retries = max(0, max_retries)
        self.retry_initial_delay = max(0.0, retry_initial_delay)
        self.retry_max_delay = max(0.0, retry_max_delay)
        self.middleware = tuple(middleware)
        self.validate_responses = validate_responses
        self.before_request = before_request
        self.after_response = after_response
        self.on_error = on_error

    def for_call(self, *, headers: dict[str, str] | None = None, timeout: float | None = None) -> Any:
        """Create an independent scope sharing middleware, hooks, auth and async driver."""
        import math
        selected_timeout = self.timeout if timeout is None else timeout
        if not math.isfinite(selected_timeout) or selected_timeout <= 0:
            raise ValueError("call timeout must be finite and positive")
        selected_headers = dict(self.headers)
        for name, value in (headers or {{}}).items():
            for previous in list(selected_headers):
                if previous.lower() == name.lower():
                    del selected_headers[previous]
            selected_headers[name] = value
        options = dict(api_key=self.api_key, bearer_token=self.bearer_token, token_provider=self.token_provider,
                       headers=selected_headers, timeout=selected_timeout, max_retries=self.max_retries,
                       retry_initial_delay=self.retry_initial_delay, retry_max_delay=self.retry_max_delay,
                       middleware=self.middleware, validate_responses=self.validate_responses,
                       before_request=self.before_request, after_response=self.after_response, on_error=self.on_error)
        if hasattr(self, "_http_client"):
            options.update(http_client=self._http_client, async_middleware=self.async_middleware)
        return type(self)(self.base_url, **options)

    def _request(
        self,
        method: str,
        path: str,
        *,
        query: dict[str, Any] | None = None,
        headers: dict[str, str] | None = None,
        body: Any = None,
        body_kind: str = "json",
        error_types: dict[int, tuple[type[ApiError], type[Any] | None]] | None = None,
        retryable: bool = False,
        idempotency_header: str | None = None,
        pagination_url: str | None = None,
        response_operation: str | None = None,
    ) -> Any:
        url = f"{{self.base_url}}{{path}}"
        if query:
            query = {{name: ([str(item).lower() if isinstance(item, bool) else item for item in value] if isinstance(value, (list, tuple)) else str(value).lower() if isinstance(value, bool) else value) for name, value in query.items()}}
            encoded = urlencode({{key: value for key, value in query.items() if value is not None}}, doseq=True)
            if encoded:
                url = f"{{url}}?{{encoded}}"
        if pagination_url is not None:
            candidate = urlsplit(urljoin(url, pagination_url))
            expected = urlsplit(url)
            if candidate.scheme != expected.scheme or candidate.netloc != expected.netloc:
                raise ValueError("pagination URL must remain on the configured API origin")
            url = candidate.geturl()
        request_headers = {{"Accept": "application/json", **self.headers, **(headers or {{}})}}
        managed_auth = self.token_provider is not None and not self.bearer_token and not any(name.lower() == "authorization" for name in request_headers)
        auth_token = self.token_provider(None) if managed_auth else None
        if auth_token:
            request_headers["Authorization"] = f"Bearer {{auth_token}}"
        elif self.bearer_token and not any(name.lower() == "authorization" for name in request_headers):
            request_headers.setdefault("Authorization", f"Bearer {{self.bearer_token}}")
        elif self.api_key and not any(name.lower() == "authorization" for name in request_headers):
            request_headers.setdefault("Authorization", f"Bearer {{self.api_key}}")
        data = None
        if body is not None:
            if isinstance(body, MultipartBody):
                if body_kind != "multipart" and not body_kind.endswith("_or_multipart"):
                    raise TypeError("operation does not declare multipart/form-data")
                body_kind = "multipart"
            elif body_kind.endswith("_or_multipart"):
                body_kind = body_kind.removesuffix("_or_multipart")
            if body_kind == "form":
                value = to_wire(body)
                if not isinstance(value, dict):
                    raise TypeError("form request bodies must serialize to a dictionary")
                request_headers.setdefault("Content-Type", "application/x-www-form-urlencoded")
                data = urlencode(value, doseq=True).encode("utf-8")
            elif body_kind == "multipart":
                data, multipart_content_type = encode_multipart(body, to_wire)
                request_headers = {{key: value for key, value in request_headers.items() if key.lower() != "content-type"}}
                request_headers["Content-Type"] = multipart_content_type
            elif body_kind in ("json-seq", "ndjson", "application/json-seq", "application/x-ndjson", "application/ndjson", "application/jsonl"):
                value = to_wire(body)
                if not isinstance(value, list):
                    raise TypeError("Sequential JSON request bodies must serialize to a list")
                request_headers.setdefault("Content-Type", body_kind if body_kind.startswith("application/") else "application/json-seq" if body_kind == "json-seq" else "application/x-ndjson")
                data = "".join(("\x1e" if body_kind in ("json-seq", "application/json-seq") else "") + json.dumps(item, allow_nan=False) + "\n" for item in value).encode("utf-8")
            elif body_kind == "binary":
                if not isinstance(body, (bytes, bytearray, memoryview)):
                    raise TypeError("binary request bodies must be bytes-like")
                request_headers.setdefault("Content-Type", "application/octet-stream")
                data = bytes(body)
            else:
                request_headers.setdefault("Content-Type", "application/json")
                data = json.dumps(to_wire(body)).encode("utf-8")
        request_context = {{"method": method, "url": url, "query": query or {{}}, "headers": request_headers, "body": body}}
        can_retry = retryable and (method.upper() in {{"GET", "HEAD", "OPTIONS", "TRACE", "QUERY", "PUT", "DELETE"}} or (method.upper() in {{"POST", "PATCH"}} and any((name.lower() == "idempotency-key" or (idempotency_header is not None and name.lower() == idempotency_header.lower())) and value is not None and bool(str(value).strip()) for name, value in request_headers.items())))
        auth_refreshed = False
        for attempt in range(self.max_retries + (2 if managed_auth else 1)):
            if self.before_request is not None:
                self.before_request({{**request_context, "attempt": attempt}})
            request = Request(url, data=data, headers=request_headers, method=method)
            try:
                with self._send(request) as response:
                    status_code = response.status
                    response_headers = dict(response.headers.items())
                    if can_retry and status_code in {{408, 429, 500, 502, 503, 504}} and attempt < self.max_retries:
                        self._retry_delay(attempt, response_headers.get("Retry-After"), next((value for name, value in response_headers.items() if name.lower() == "retry-after-ms"), None))
                        continue
                    raw = response.read(10 * 1024 * 1024 + 1) if self.validate_responses else response.read()
                    if self.after_response is not None:
                        self.after_response({{"request": request_context, "status_code": status_code, "headers": response_headers, "body": raw}})
                    content_type = response.headers.get_content_type()
                    from .response_validation import decode_json, decode_sequence
                    decoded = decode_sequence(raw, content_type, self.validate_responses) if raw and content_type in ("application/json-seq", "application/x-ndjson", "application/ndjson", "application/jsonl") else decode_json(raw, self.validate_responses) if raw and (content_type == "application/json" or content_type.endswith("+json")) else raw or None
                    if self.validate_responses and response_operation is not None and method.upper() != "HEAD":
                        from .response_validation import PLANS, check_response
                        check_response(decoded, PLANS["operations"].get(response_operation, {{}}), PLANS["refs"], status_code, content_type)
                    return decoded
            except ResponseDecodeError as error:
                if self.on_error is not None:
                    self.on_error(error, request_context)
                raise
            except HTTPError as error:
                if error.code == 401 and managed_auth and can_retry and not auth_refreshed:
                    error.close()
                    auth_token = self.token_provider(auth_token)
                    request_headers["Authorization"] = f"Bearer {{auth_token}}"
                    auth_refreshed = True
                    continue
                if can_retry and error.code in {{408, 429, 500, 502, 503, 504}} and attempt < self.max_retries:
                    retry_after = error.headers.get("Retry-After")
                    error.close()
                    self._retry_delay(attempt, retry_after, error.headers.get("retry-after-ms"))
                    continue
                try:
                    raw = error.read()
                finally:
                    error.close()
                try:
                    response_body = json.loads(raw) if raw else None
                except json.JSONDecodeError:
                    response_body = raw.decode("utf-8", errors="replace")
                error_type, body_type = (error_types or {{}}).get(error.code, (ApiError, None))
                if body_type is not None and isinstance(response_body, dict):
                    response_body = body_type.from_dict(response_body)
                raised = error_type(error.code, dict(error.headers.items()), response_body)
                if self.on_error is not None:
                    self.on_error(raised, request_context)
                raise raised from error
            except URLError as error:
                if can_retry and attempt < self.max_retries:
                    self._retry_delay(attempt)
                    continue
                if self.on_error is not None:
                    self.on_error(error, request_context)
                raise
        raise RuntimeError("Poolster retry loop completed without a response")

    def _send(self, request: Request) -> Any:
        """Run native transport middleware in declaration order for each attempt."""
        def transport(request: Request) -> Any:
            return urlopen(request, timeout=self.timeout)
        handler = transport
        for middleware in reversed(self.middleware):
            following = handler
            def handler(request: Request, middleware: Any = middleware, following: Any = following) -> Any:
                return middleware(request, following)
        return handler(request)

    def _retry_delay(self, attempt: int, retry_after: str | None = None, retry_after_ms: str | None = None) -> None:
        import math
        server_delay = None
        for value, divisor in ((retry_after_ms, 1000.0), (retry_after, 1.0)):
            try:
                candidate = float(value) / divisor if value is not None else None
            except (ValueError, TypeError):
                continue
            if candidate is not None and math.isfinite(candidate) and candidate >= 0:
                server_delay = candidate
                break
        delay = server_delay if server_delay is not None else self.retry_initial_delay * (2 ** attempt)
        time.sleep(min(max(0.0, delay), max(0.0, self.retry_max_delay)))

    def _event_stream(
        self,
        method: str,
        path: str,
        *,
        query: dict[str, Any] | None = None,
        headers: dict[str, str] | None = None,
        body: Any = None,
        body_kind: str = "json",
        error_types: dict[int, tuple[type[ApiError], type[Any] | None]] | None = None,
        retryable: bool = False,
        idempotency_header: str | None = None,
    ) -> Iterator[Any]:
        """Open an SSE response and yield decoded ``data:`` events.

        Retries are limited to establishing the connection. Once an event has
        been yielded, reconnecting is application policy: Poolster never invents a
        last-event-id or resume token.
        """
        url = f"{{self.base_url}}{{path}}"
        if query:
            encoded_query = urlencode({{key: value for key, value in query.items() if value is not None}}, doseq=True)
            if encoded_query:
                url = f"{{url}}?{{encoded_query}}"
        request_headers = {{"Accept": "text/event-stream", **self.headers, **(headers or {{}})}}
        managed_auth = self.token_provider is not None and not self.bearer_token and not any(name.lower() == "authorization" for name in request_headers)
        auth_token = self.token_provider(None) if managed_auth else None
        if auth_token:
            request_headers["Authorization"] = f"Bearer {{auth_token}}"
        elif self.bearer_token and not any(name.lower() == "authorization" for name in request_headers):
            request_headers.setdefault("Authorization", f"Bearer {{self.bearer_token}}")
        elif self.api_key and not any(name.lower() == "authorization" for name in request_headers):
            request_headers.setdefault("Authorization", f"Bearer {{self.api_key}}")
        data = None
        if body is not None:
            if isinstance(body, MultipartBody):
                if body_kind != "multipart" and not body_kind.endswith("_or_multipart"):
                    raise TypeError("operation does not declare multipart/form-data")
                body_kind = "multipart"
            elif body_kind.endswith("_or_multipart"):
                body_kind = body_kind.removesuffix("_or_multipart")
            if body_kind == "form":
                value = to_wire(body)
                if not isinstance(value, dict):
                    raise TypeError("form request bodies must serialize to a dictionary")
                request_headers.setdefault("Content-Type", "application/x-www-form-urlencoded")
                data = urlencode(value, doseq=True).encode("utf-8")
            elif body_kind == "multipart":
                data, multipart_content_type = encode_multipart(body, to_wire)
                request_headers = {{key: value for key, value in request_headers.items() if key.lower() != "content-type"}}
                request_headers["Content-Type"] = multipart_content_type
            elif body_kind == "binary":
                if not isinstance(body, (bytes, bytearray, memoryview)):
                    raise TypeError("binary request bodies must be bytes-like")
                request_headers.setdefault("Content-Type", "application/octet-stream")
                data = bytes(body)
            else:
                request_headers.setdefault("Content-Type", "application/json")
                data = json.dumps(to_wire(body)).encode("utf-8")
        context = {{"method": method, "url": url, "query": query or {{}}, "headers": request_headers, "body": body}}
        can_retry = retryable and (method.upper() in {{"GET", "HEAD", "OPTIONS", "TRACE", "QUERY", "PUT", "DELETE"}} or (method.upper() in {{"POST", "PATCH"}} and any((name.lower() == "idempotency-key" or (idempotency_header is not None and name.lower() == idempotency_header.lower())) and value is not None and bool(str(value).strip()) for name, value in request_headers.items())))
        auth_refreshed = False
        for attempt in range(self.max_retries + (2 if managed_auth else 1)):
            if self.before_request is not None:
                self.before_request({{**context, "attempt": attempt}})
            request = Request(url, data=data, headers=request_headers, method=method)
            try:
                response = self._send(request)
                if self.after_response is not None:
                    self.after_response({{"request": context, "status_code": response.status, "headers": dict(response.headers.items()), "body": None}})
                def events() -> Iterator[Any]:
                    data_lines: list[str] = []
                    try:
                        for raw_line in response:
                            line = raw_line.decode("utf-8", errors="replace").rstrip("\r\n")
                            if line == "":
                                if data_lines:
                                    payload = "\n".join(data_lines)
                                    try:
                                        yield json.loads(payload)
                                    except json.JSONDecodeError:
                                        yield payload
                                    data_lines = []
                            elif line.startswith("data:"):
                                data_lines.append(line[5:].lstrip(" "))
                        if data_lines:
                            payload = "\n".join(data_lines)
                            try:
                                yield json.loads(payload)
                            except json.JSONDecodeError:
                                yield payload
                    finally:
                        response.close()
                return events()
            except HTTPError as error:
                if error.code == 401 and managed_auth and can_retry and not auth_refreshed:
                    error.close()
                    auth_token = self.token_provider(auth_token)
                    request_headers["Authorization"] = f"Bearer {{auth_token}}"
                    auth_refreshed = True
                    continue
                if can_retry and error.code in {{408, 429, 500, 502, 503, 504}} and attempt < self.max_retries:
                    retry_after = error.headers.get("Retry-After")
                    error.close()
                    self._retry_delay(attempt, retry_after, error.headers.get("retry-after-ms"))
                    continue
                try:
                    raw = error.read()
                finally:
                    error.close()
                try:
                    response_body = json.loads(raw) if raw else None
                except json.JSONDecodeError:
                    response_body = raw.decode("utf-8", errors="replace")
                error_type, body_type = (error_types or {{}}).get(error.code, (ApiError, None))
                if body_type is not None and isinstance(response_body, dict):
                    response_body = body_type.from_dict(response_body)
                raised = error_type(error.code, dict(error.headers.items()), response_body)
                if self.on_error is not None:
                    self.on_error(raised, context)
                raise raised from error
            except URLError as error:
                if can_retry and attempt < self.max_retries:
                    self._retry_delay(attempt)
                    continue
                if self.on_error is not None:
                    self.on_error(error, context)
                raise
        raise RuntimeError("Poolster stream retry loop completed without a response")

"#,
        api.name
    )
}
