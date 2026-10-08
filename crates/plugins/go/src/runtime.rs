use super::*;

pub(super) fn render_runtime(api: &Api, package: &str, client_style: SdkClientStyle) -> String {
    let resources = resource_operations(api);
    let facades = resource_facade_names(&resources);
    let facade_fields = if client_style == SdkClientStyle::Namespaced {
        resources
            .keys()
            .map(|resource| {
                let facade = &facades[resource];
                format!("\t{facade} *{facade}Service\n")
            })
            .collect::<String>()
    } else {
        String::new()
    };
    let facade_setup = if client_style == SdkClientStyle::Namespaced {
        resources
            .keys()
            .map(|resource| {
                let facade = &facades[resource];
                format!("\tclient.{facade} = &{facade}Service{{client: client}}\n")
            })
            .collect::<String>()
    } else {
        String::new()
    };
    let mut output = format!(
        "{NOTICE}\npackage {package}\n\nimport (\n\t\"bytes\"\n\t\"context\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"io\"\n\t\"net/http\"\n\t\"net/url\"\n\t\"reflect\"\n\t\"strings\"\n)\n\n// ClientConfig configures a generated API client.\ntype ClientConfig struct {{\n\tBaseURL      string\n\tAPIKey       string\n\tAPIKeyHeader string\n\tAPIKeyPrefix string\n\tHTTPClient   *http.Client\n}}\n\n// Client is safe for concurrent use after construction.\ntype Client struct {{\n\tbaseURL      string\n\tapiKey       string\n\tapiKeyHeader string\n\tapiKeyPrefix string\n\thttpClient   *http.Client\n{facade_fields}}}\n\n// NewClient builds a client from explicit configuration. BaseURL is required.\nfunc NewClient(config ClientConfig) (*Client, error) {{\n\tbaseURL := strings.TrimRight(config.BaseURL, \"/\")\n\tif baseURL == \"\" {{\n\t\treturn nil, fmt.Errorf(\"poolster: BaseURL is required\")\n\t}}\n\theader := config.APIKeyHeader\n\tif header == \"\" {{\n\t\theader = \"Authorization\"\n\t}}\n\thttpClient := config.HTTPClient\n\tif httpClient == nil {{\n\t\thttpClient = http.DefaultClient\n\t}}\n\tclient := &Client{{baseURL: baseURL, apiKey: config.APIKey, apiKeyHeader: header, apiKeyPrefix: config.APIKeyPrefix, httpClient: httpClient}}\n{facade_setup}\treturn client, nil\n}}\n\n// APIError describes a non-success HTTP response.\ntype APIError struct {{\n\tStatusCode int\n\tBody       string\n}}\n\nfunc (errorResponse *APIError) Error() string {{\n\treturn fmt.Sprintf(\"poolster: API request failed with status %d: %s\", errorResponse.StatusCode, errorResponse.Body)\n}}\n\nfunc (client *Client) newRequest(ctx context.Context, method, path string, query url.Values, headers http.Header, body any) (*http.Request, error) {{\n\tendpoint := client.baseURL + path\n\tif len(query) > 0 {{\n\t\tendpoint += \"?\" + query.Encode()\n\t}}\n\tvar reader io.Reader\n\tif body != nil {{\n\t\tencoded, err := json.Marshal(body)\n\t\tif err != nil {{\n\t\t\treturn nil, fmt.Errorf(\"poolster: encode request body: %w\", err)\n\t\t}}\n\t\treader = bytes.NewReader(encoded)\n\t}}\n\trequest, err := http.NewRequestWithContext(ctx, method, endpoint, reader)\n\tif err != nil {{\n\t\treturn nil, fmt.Errorf(\"poolster: build request: %w\", err)\n\t}}\n\trequest.Header.Set(\"Accept\", \"application/json\")\n\tif body != nil {{\n\t\trequest.Header.Set(\"Content-Type\", \"application/json\")\n\t}}\n\tfor name, values := range headers {{\n\t\tfor _, value := range values {{\n\t\t\trequest.Header.Add(name, value)\n\t\t}}\n\t}}\n\tif client.apiKey != \"\" {{\n\t\tcredential := client.apiKey\n\t\tif client.apiKeyPrefix != \"\" {{\n\t\t\tcredential = client.apiKeyPrefix + \" \" + credential\n\t\t}}\n\t\trequest.Header.Set(client.apiKeyHeader, credential)\n\t}}\n\treturn request, nil\n}}\n\nfunc (client *Client) do(request *http.Request, destination any) error {{\n\tresponse, err := client.httpClient.Do(request)\n\tif err != nil {{\n\t\treturn fmt.Errorf(\"poolster: execute request: %w\", err)\n\t}}\n\tdefer response.Body.Close()\n\tif response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {{\n\t\tbody, _ := io.ReadAll(io.LimitReader(response.Body, 1<<20))\n\t\treturn &APIError{{StatusCode: response.StatusCode, Body: string(body)}}\n\t}}\n\tif destination == nil || response.StatusCode == http.StatusNoContent {{\n\t\treturn nil\n\t}}\n\tif err := json.NewDecoder(response.Body).Decode(destination); err != nil && err != io.EOF {{\n\t\treturn fmt.Errorf(\"poolster: decode response: %w\", err)\n\t}}\n\treturn nil\n}}\n\nfunc addQuery(query url.Values, name string, value any) error {{\n\tif value == nil {{\n\t\treturn nil\n\t}}\n\treflected := reflect.ValueOf(value)\n\tif reflected.Kind() == reflect.Pointer {{\n\t\tif reflected.IsNil() {{\n\t\t\treturn nil\n\t\t}}\n\t\treturn addQuery(query, name, reflected.Elem().Interface())\n\t}}\n\tif reflected.Kind() == reflect.Slice || reflected.Kind() == reflect.Array {{\n\t\tfor index := 0; index < reflected.Len(); index++ {{\n\t\t\tif err := addQuery(query, name, reflected.Index(index).Interface()); err != nil {{\n\t\t\t\treturn err\n\t\t\t}}\n\t\t}}\n\t\treturn nil\n\t}}\n\tswitch value.(type) {{\n\tcase string, bool, int, int8, int16, int32, int64, uint, uint8, uint16, uint32, uint64, float32, float64:\n\t\tquery.Add(name, fmt.Sprint(value))\n\t\treturn nil\n\tdefault:\n\t\tencoded, err := json.Marshal(value)\n\t\tif err != nil {{\n\t\t\treturn fmt.Errorf(\"poolster: encode query %s: %w\", name, err)\n\t\t}}\n\t\tquery.Add(name, string(encoded))\n\t\treturn nil\n\t}}\n\n"
    );
    output = output.replace(
        "func (client *Client) newRequest(ctx context.Context, method, path string, query url.Values, headers http.Header, body any) (*http.Request, error) {\n\tendpoint := client.baseURL + path\n\tif len(query) > 0 {\n\t\tendpoint += \"?\" + query.Encode()\n\t}\n\tvar reader io.Reader",
        "func (client *Client) newRequest(ctx context.Context, method, path string, query url.Values, headers http.Header, body any) (*http.Request, error) {\n\tendpoint := client.baseURL + path\n\tif len(query) > 0 { endpoint += \"?\" + query.Encode() }\n\treturn client.newRequestURL(ctx, method, endpoint, headers, body)\n}\n\n// newPaginationRequest is private on purpose: a generated pager may follow\n// only a same-origin URL supplied by its declared response contract. It still\n// uses the normal request builder, so auth, caller headers, and body encoding\n// cannot be bypassed by a continuation link.\nfunc (client *Client) newPaginationRequest(ctx context.Context, method, continuation string, headers http.Header, body any) (*http.Request, error) {\n\tbase, err := url.Parse(client.baseURL)\n\tif err != nil { return nil, fmt.Errorf(\"poolster: parse BaseURL: %w\", err) }\n\tnext, err := url.Parse(continuation)\n\tif err != nil { return nil, fmt.Errorf(\"poolster: parse pagination URL: %w\", err) }\n\tendpoint := base.ResolveReference(next)\n\tif endpoint.Scheme != base.Scheme || endpoint.Host != base.Host {\n\t\treturn nil, fmt.Errorf(\"poolster: pagination URL must remain on the configured API origin\")\n\t}\n\treturn client.newRequestURL(ctx, method, endpoint.String(), headers, body)\n}\n\nfunc (client *Client) newRequestURL(ctx context.Context, method, endpoint string, headers http.Header, body any) (*http.Request, error) {\n\tvar reader io.Reader",
    );
    // A spec can legitimately define a model called APIError. Keep the
    // transport error private so it can never collide with exported models.
    output = output.replace("APIError", "poolsterAPIError");
    // The format literal closes `addQuery`'s switch; this closes the helper.
    output.push_str("}\n\n");

    if api.operations.iter().any(|operation| {
        cursor_pagination(api, operation).is_some()
            || page_pagination::supported(api, operation)
            || offset_pagination(api, operation).is_some()
            || url_pagination(api, operation).is_some()
    }) {
        output.push_str(PAGINATION_RUNTIME);
    }
    let mut output = add_retry_runtime(output);
    output = output.replace("\t\"reflect\"\n", "\t\"reflect\"\n\t\"sort\"\n");
    output.push_str(include_str!("../templates/openapi32.go.tmpl"));
    output = output
        .replace(
            "HTTPClient   *http.Client",
            "HTTPClient   PoolsterHTTPClient",
        )
        .replace(
            "httpClient   *http.Client",
            "httpClient   PoolsterHTTPClient",
        );
    output = output.replace("HTTPClient   PoolsterHTTPClient\n", "HTTPClient   PoolsterHTTPClient\n\t// Middleware wraps each transport attempt; first configured is outermost.\n\tMiddleware []PoolsterMiddleware\n");
    output = output.replace("client := &Client{baseURL:", "for index := len(config.Middleware) - 1; index >= 0; index-- {\n\t\tif config.Middleware[index] == nil { return nil, fmt.Errorf(\"poolster: nil middleware\") }\n\t\thttpClient = config.Middleware[index](httpClient)\n\t\tif httpClient == nil { return nil, fmt.Errorf(\"poolster: middleware returned nil transport\") }\n\t}\n\tclient := &Client{baseURL:");
    output = output.replace("Middleware []PoolsterMiddleware\n", "Middleware []PoolsterMiddleware\n\t// ValidateResponses checks named model shapes in buffered JSON responses.\n\tValidateResponses bool\n");
    output = output.replace(
        "hooks        PoolsterClientHooks\n",
        "hooks        PoolsterClientHooks\n\tvalidateResponses bool\n",
    );
    output = output.replace(
        "hooks: config.Hooks}",
        "hooks: config.Hooks, validateResponses: config.ValidateResponses}",
    );
    if api.operations.iter().any(|operation| {
        operation
            .annotations
            .contains_key("x-poolster-idempotency-resolved")
    }) {
        output = output.replace("\t\"context\"\n", "\t\"context\"\n\t\"crypto/rand\"\n");
        output = output.replace("return strings.TrimSpace(request.Header.Get(\"Idempotency-Key\")) != \"\"", "header, _ := request.Context().Value(poolsterIdempotencyContextKey{}).(string)\n\t\treturn strings.TrimSpace(request.Header.Get(\"Idempotency-Key\")) != \"\" || (header != \"\" && strings.TrimSpace(request.Header.Get(header)) != \"\")");
        output.push_str(include_str!("../templates/idempotency.go.tmpl"));
    }
    output.push_str(&response_validation::render(api));
    output.push_str(include_str!("../templates/middleware.go.tmpl"));
    output = output.replace(
        "\treturn request, nil\n}",
        "\tapplyCallHeaders(request)\n\treturn request, nil\n}",
    );
    output.push_str(include_str!("../templates/call_options.go.tmpl"));
    output = output.replace(
        "\tvar reader io.Reader\n",
        "\tvar reader io.Reader\n\tcontentType := \"application/json\"\n",
    );
    output = output.replace(
        "encoded, err := json.Marshal(body)",
        "encoded, selectedType, err := encodePoolsterWireBody(body)",
    );
    output = output.replace(
        "reader = bytes.NewReader(encoded)",
        "reader = bytes.NewReader(encoded)\n\t\tcontentType = selectedType",
    );
    output = output.replace(
        "request.Header.Set(\"Content-Type\", \"application/json\")",
        "request.Header.Set(\"Content-Type\", contentType)",
    );
    if api.operations.iter().any(has_multipart) {
        output.push_str(include_str!("../templates/multipart.go.tmpl"));
    }
    output.push_str("\nfunc encodePoolsterWireBody(body any) ([]byte,string,error) { if sequential,ok:=body.(poolsterSequentialBody);ok {return encodePoolsterSequential(sequential)}\n");
    if api.operations.iter().any(has_multipart) {
        output.push_str("return encodePoolsterBody(body)\n}\n");
    } else {
        output.push_str(
            "encoded,err:=json.Marshal(body);return encoded,\"application/json\",err\n}\n",
        );
    }
    output = output.replace("ValidateResponses bool\n", "ValidateResponses bool\n\t// TokenProvider supplies managed bearer tokens; explicit Authorization wins.\n\tTokenProvider PoolsterTokenProvider\n");
    output = output.replace(
        "validateResponses bool\n",
        "validateResponses bool\n\ttokenProvider PoolsterTokenProvider\n",
    );
    output = output.replace(
        "validateResponses: config.ValidateResponses}",
        "validateResponses: config.ValidateResponses, tokenProvider: config.TokenProvider}",
    );
    output.push_str("\n// PoolsterTokenProvider supplies a token, refreshing only a rejected cached token.\ntype PoolsterTokenProvider interface { Token(context.Context, string) (string, error) }\n");
    output.push_str("\n// PoolsterHTTPClient is the replaceable execution boundary; context cancellation remains on the request.\ntype PoolsterHTTPClient interface { Do(*http.Request) (*http.Response, error) }\n");
    output
}

pub(super) fn add_retry_runtime(mut output: String) -> String {
    let needs_errors = output.contains("errors.As(")
        || include_str!("../templates/response_validation.go.tmpl").contains("errors.Is(");
    output = output.replace(
        "\t\"strings\"\n)",
        if needs_errors {
            "\t\"errors\"\n\t\"strconv\"\n\t\"strings\"\n\t\"time\"\n)"
        } else {
            "\t\"strconv\"\n\t\"strings\"\n\t\"time\"\n)"
        },
    );
    output = output.replace(
        "\tBody       string\n",
        "\tBody        []byte\n\tContentType string\n",
    );
    output = output.replace(
        "errorResponse.StatusCode, errorResponse.Body)",
        "errorResponse.StatusCode, string(errorResponse.Body))",
    );
    output = output.replace(
        "Body: string(body)",
        "Body: body, ContentType: response.Header.Get(\"Content-Type\")",
    );
    output = output.replace(
        "\tHTTPClient   *http.Client\n}",
        "\tHTTPClient   *http.Client\n\t// Retry configures safe automatic retries. Nil uses Poolster defaults; set\n\t// MaxAttempts to 1 to disable retries.\n\tRetry        *RetryConfig\n\t// Hooks observes the final lifecycle of a logical request. It receives no\n\t// request headers or bodies, so credentials stay private by default.\n\tHooks        PoolsterClientHooks\n}",
    );
    output = output.replace(
        "\thttpClient   *http.Client\n",
        "\thttpClient   *http.Client\n\tretry        retryConfig\n\thooks        PoolsterClientHooks\n",
    );
    output = output.replace(
        "client := &Client{baseURL: baseURL, apiKey: config.APIKey, apiKeyHeader: header, apiKeyPrefix: config.APIKeyPrefix, httpClient: httpClient}",
        "client := &Client{baseURL: baseURL, apiKey: config.APIKey, apiKeyHeader: header, apiKeyPrefix: config.APIKeyPrefix, httpClient: httpClient, retry: normalizeRetry(config.Retry), hooks: config.Hooks}",
    );
    output = output.replace("client.do(request,", "client.doWithRetry(request,");
    output.push_str(include_str!("../templates/retry.go.tmpl"));
    output
}
