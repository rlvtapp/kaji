package main

type OperationDoc struct {
	Kind        string   `json:"kind,omitempty"`
	Name        string   `json:"name,omitempty"`
	OperationID string   `json:"operation_id,omitempty"`
	Path        string   `json:"path"`
	Method      string   `json:"method"`
	Summary     string   `json:"summary,omitempty"`
	Description string   `json:"description,omitempty"`
	Deprecated  bool     `json:"deprecated,omitempty"`
	Hidden      bool     `json:"hidden,omitempty"`
	Auth        *AuthDoc `json:"auth,omitempty"`
	// SecurityRequirements is the lossless OpenAPI security shape: each entry
	// is an alternative and the named schemes inside it are required together.
	// Auth remains as the legacy, documentation-oriented summary.
	SecurityRequirements []SecurityRequirementDoc `json:"security_requirements,omitempty"`
	Servers              []ServerDoc              `json:"servers,omitempty"`
	Parameters           []ParameterDoc           `json:"parameters,omitempty"`
	RequestBody          *BodyDoc                 `json:"request_body,omitempty"`
	Responses            []ResponseDoc            `json:"responses,omitempty"`
	RequestExamples      []ExampleDoc             `json:"request_examples,omitempty"`
	Extensions           map[string]any           `json:"extensions,omitempty"`
}

type BodyDoc struct {
	Description      string        `json:"description,omitempty"`
	Required         bool          `json:"required,omitempty"`
	ContentType      string        `json:"content_type,omitempty"`
	Schema           []SchemaField `json:"schema,omitempty"`
	SchemaDefinition any           `json:"schema_definition,omitempty"`
	ExampleJSON      string        `json:"example_json,omitempty"`
	// MediaTypes preserves every request content entry. The fields above retain
	// the first representation for existing docs consumers.
	MediaTypes []MediaTypeDoc `json:"media_types,omitempty"`
}

// MediaTypeDoc is the shared lossless representation of a request or response
// content entry for downstream SDK generators.
type MediaTypeDoc struct {
	ContentType      string        `json:"content_type"`
	Schema           []SchemaField `json:"schema,omitempty"`
	SchemaDefinition any           `json:"schema_definition,omitempty"`
	ExampleJSON      string        `json:"example_json,omitempty"`
}

type AuthDoc struct {
	Required    bool   `json:"required"`
	Scheme      string `json:"scheme,omitempty"`
	Description string `json:"description,omitempty"`
	Type        string `json:"type,omitempty"`
	Name        string `json:"name,omitempty"`
	In          string `json:"in,omitempty"`
	HttpScheme  string `json:"http_scheme,omitempty"`
}

// SecurityRequirementDoc preserves the names and scopes from one OpenAPI
// Security Requirement Object. Multiple objects are alternatives (OR), while
// multiple schemes in one object are conjunctive (AND).
type SecurityRequirementDoc struct {
	Schemes map[string][]string `json:"schemes"`
}

type ParameterDoc struct {
	Name        string `json:"name"`
	In          string `json:"in"`
	Required    bool   `json:"required"`
	Description string `json:"description,omitempty"`
	Style       string `json:"style,omitempty"`
	Explode     *bool  `json:"explode,omitempty"`
	Schema      any    `json:"schema,omitempty"`
}

type ResponseDoc struct {
	Code             string        `json:"code"`
	Description      string        `json:"description,omitempty"`
	ContentType      string        `json:"content_type,omitempty"`
	Schema           []SchemaField `json:"schema,omitempty"`
	SchemaDefinition any           `json:"schema_definition,omitempty"`
	ExampleJSON      string        `json:"example_json,omitempty"`
}

type ExampleDoc struct {
	Label       string `json:"label"`
	ContentType string `json:"content_type,omitempty"`
	ExampleJSON string `json:"example_json,omitempty"`
}

type ServerDoc struct {
	Name        string              `json:"name,omitempty"`
	URL         string              `json:"url"`
	Description string              `json:"description,omitempty"`
	Variables   []ServerVariableDoc `json:"variables,omitempty"`
}

type ServerVariableDoc struct {
	Name        string   `json:"name"`
	Default     string   `json:"default,omitempty"`
	Enum        []string `json:"enum,omitempty"`
	Description string   `json:"description,omitempty"`
}

type SchemaField struct {
	Name        string        `json:"name"`
	Type        string        `json:"type,omitempty"`
	Description string        `json:"description,omitempty"`
	Enum        []string      `json:"enum,omitempty"`
	Children    []SchemaField `json:"children,omitempty"`
}

// ComponentSchemaDoc is the lossless component-schema boundary for consumers
// that need to generate SDKs. Schema is normalized to JSON-compatible values
// so downstream tooling does not need to embed or understand the Go OpenAPI
// library.
type ComponentSchemaDoc struct {
	Name   string `json:"name"`
	Schema any    `json:"schema"`
}

type ComponentSchemasDoc struct {
	Schemas []ComponentSchemaDoc `json:"schemas"`
}

// SecuritySchemesDoc is the component security-scheme catalog for SDK
// generators. Operation security requirements only identify scheme names and
// requested scopes; this companion artifact carries the transport metadata
// needed to apply those requirements.
type SecuritySchemesDoc struct {
	Schemes []SecuritySchemeDoc `json:"schemes"`
}

// SecuritySchemeDoc is a JSON-compatible OpenAPI Security Scheme Object.
// The fields intentionally cover the metadata needed for apiKey, http, and
// oauth2 SDK auth while retaining OpenID discovery metadata for consumers that
// support it.
type SecuritySchemeDoc struct {
	Name              string         `json:"name"`
	Type              string         `json:"type"`
	Description       string         `json:"description,omitempty"`
	APIKeyName        string         `json:"api_key_name,omitempty"`
	APIKeyIn          string         `json:"api_key_in,omitempty"`
	HTTPScheme        string         `json:"http_scheme,omitempty"`
	BearerFormat      string         `json:"bearer_format,omitempty"`
	OAuthFlows        []OAuthFlowDoc `json:"oauth_flows,omitempty"`
	OpenIDConnectURL  string         `json:"open_id_connect_url,omitempty"`
	OAuth2MetadataURL string         `json:"oauth2_metadata_url,omitempty"`
}

// OAuthFlowDoc preserves a named OAuth2 flow and all URL/scope metadata from
// OpenAPI. Scopes use a map because scope names are identifiers, and Go's JSON
// encoder emits map keys in a stable order.
type OAuthFlowDoc struct {
	Type             string            `json:"type"`
	AuthorizationURL string            `json:"authorization_url,omitempty"`
	TokenURL         string            `json:"token_url,omitempty"`
	RefreshURL       string            `json:"refresh_url,omitempty"`
	Scopes           map[string]string `json:"scopes,omitempty"`
}
