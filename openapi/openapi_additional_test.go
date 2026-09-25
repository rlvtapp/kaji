package main

import (
	"encoding/json"
	"testing"

	v3 "github.com/pb33f/libopenapi/datamodel/high/v3"
)

const securitySpec = `openapi: 3.1.0
info:
  title: Secure
  version: 1.0.0
components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
      description: Use a bearer token
security:
  - BearerAuth: []
paths:
  /secure:
    get:
      responses:
        '200':
          description: OK
`

func TestSlugForNormalizesPath(t *testing.T) {
	if got := slugFor("/items/{id}", "post"); got != "post_items_id" {
		t.Fatalf("unexpected slug: %s", got)
	}
	if got := slugFor("/", "get"); got != "get_root" {
		t.Fatalf("unexpected slug: %s", got)
	}
}

func TestParamDocKeyLowercases(t *testing.T) {
	key := paramDocKey(ParameterDoc{Name: "UserId", In: "HEADER"})
	if key != "header:userid" {
		t.Fatalf("unexpected key: %s", key)
	}
}

func TestMergeParametersOverrides(t *testing.T) {
	pathParams := []ParameterDoc{{Name: "id", In: "path", Required: true}}
	opParams := []ParameterDoc{{Name: "id", In: "path", Required: false}}

	merged := mergeParameters(pathParams, opParams)
	if len(merged) != 1 {
		t.Fatalf("expected 1 param, got %d", len(merged))
	}
	if merged[0].Required {
		t.Fatalf("expected op param to override required")
	}
}

func TestConvertParameterDefaultsRequiredForPath(t *testing.T) {
	param := &v3.Parameter{Name: "id", In: "path"}
	converted, err := convertParameter(param)
	if err != nil {
		t.Fatalf("convert parameter: %v", err)
	}
	if !converted.Required {
		t.Fatalf("expected path param to be required")
	}
}

func TestFormatExampleJSON(t *testing.T) {
	if got := formatExampleJSON("hello"); got != "\"hello\"" {
		t.Fatalf("unexpected string json: %s", got)
	}

	obj := map[string]any{"count": "<number>"}
	encoded := formatExampleJSON(obj)
	var decoded map[string]any
	if err := json.Unmarshal([]byte(encoded), &decoded); err != nil {
		t.Fatalf("encoded json invalid: %v", err)
	}
}

func TestConvertRequestExamplesFromRequestBody(t *testing.T) {
	spec := loadSpec(t, `openapi: 3.1.0
info:
  title: Example
  version: 1.0.0
paths:
  /widgets:
    post:
      requestBody:
        content:
          application/json:
            example:
              name: widget
              enabled: true
      responses:
        '200':
          description: OK
`)

	operation := spec.Paths.PathItems.GetOrZero("/widgets").Post
	if operation == nil {
		t.Fatalf("post operation missing")
	}

	examples, err := convertRequestExamples(operation.RequestBody)
	if err != nil {
		t.Fatalf("convert request examples: %v", err)
	}
	if len(examples) != 1 {
		t.Fatalf("expected 1 request example, got %d", len(examples))
	}
	if examples[0].ExampleJSON == "" {
		t.Fatalf("expected request example json")
	}
}

func TestConvertRequestExamplesFromRequestBodyExamplesMap(t *testing.T) {
	spec := loadSpec(t, `openapi: 3.1.0
info:
  title: Example
  version: 1.0.0
paths:
  /widgets:
    post:
      requestBody:
        content:
          application/json:
            examples:
              default:
                value:
                  name: widget
                  enabled: true
      responses:
        '200':
          description: OK
`)

	operation := spec.Paths.PathItems.GetOrZero("/widgets").Post
	if operation == nil {
		t.Fatalf("post operation missing")
	}

	examples, err := convertRequestExamples(operation.RequestBody)
	if err != nil {
		t.Fatalf("convert request examples: %v", err)
	}
	if len(examples) != 1 {
		t.Fatalf("expected 1 request example, got %d", len(examples))
	}
	if examples[0].ExampleJSON == "" {
		t.Fatalf("expected request example json")
	}
	if examples[0].Label != "Request application/json: default" {
		t.Fatalf("unexpected example label: %q", examples[0].Label)
	}
}

func TestConvertRequestExamplesPreservesEveryNamedExample(t *testing.T) {
	spec := loadSpec(t, `openapi: 3.1.0
info:
  title: Example
  version: 1.0.0
paths:
  /widgets:
    post:
      requestBody:
        content:
          application/json:
            examples:
              minimal:
                value:
                  name: widget
              complete:
                value:
                  name: widget
                  enabled: true
      responses:
        '200':
          description: OK
`)

	operation := spec.Paths.PathItems.GetOrZero("/widgets").Post
	examples, err := convertRequestExamples(operation.RequestBody)
	if err != nil {
		t.Fatalf("convert request examples: %v", err)
	}
	if len(examples) != 2 {
		t.Fatalf("expected 2 named request examples, got %d", len(examples))
	}
	if examples[0].Label != "Request application/json: minimal" || examples[1].Label != "Request application/json: complete" {
		t.Fatalf("named examples lost their order or labels: %#v", examples)
	}
	if examples[0].ExampleJSON == examples[1].ExampleJSON {
		t.Fatalf("distinct named examples were collapsed: %#v", examples)
	}
}

func TestGenerateExampleFromSchemaPrefersSchemaExample(t *testing.T) {
	spec := loadSpec(t, `openapi: 3.1.0
info:
  title: Example
  version: 1.0.0
paths:
  /widgets:
    get:
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: object
                example:
                  name: widget
                  enabled: true
`)

	operation := spec.Paths.PathItems.GetOrZero("/widgets").Get
	if operation == nil {
		t.Fatalf("get operation missing")
	}

	responses, err := convertResponses(operation.Responses)
	if err != nil {
		t.Fatalf("convert responses: %v", err)
	}
	if len(responses) == 0 {
		t.Fatalf("expected response docs")
	}
	if responses[0].ExampleJSON == "" {
		t.Fatalf("expected schema example json")
	}
}

func TestGenerateExampleFromSchemaUsesSchemaExamplesList(t *testing.T) {
	spec := loadSpec(t, `openapi: 3.1.0
info:
  title: Example
  version: 1.0.0
paths:
  /widgets:
    get:
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: object
                examples:
                  - name: widget
                    enabled: true
`)

	operation := spec.Paths.PathItems.GetOrZero("/widgets").Get
	if operation == nil {
		t.Fatalf("get operation missing")
	}

	responses, err := convertResponses(operation.Responses)
	if err != nil {
		t.Fatalf("convert responses: %v", err)
	}
	if len(responses) == 0 {
		t.Fatalf("expected response docs")
	}
	if responses[0].ExampleJSON == "" {
		t.Fatalf("expected schema examples json")
	}
}

func TestComputeSpecHashChanges(t *testing.T) {
	first := computeSpecHash([]byte("one"))
	second := computeSpecHash([]byte("two"))
	if first == second {
		t.Fatalf("hash should change for different inputs")
	}
}

func TestBuildAuthDocUsesSchemeDescription(t *testing.T) {
	spec := loadSampleSpec(t, securitySpec)
	_, op := getOperation(t, spec, "get")

	auth := buildAuthDoc(op.Security, spec.Security, spec.Components)
	if auth == nil {
		t.Fatalf("expected auth doc")
	}
	if !auth.Required {
		t.Fatalf("expected auth required")
	}
	if auth.Scheme != "BearerAuth" {
		t.Fatalf("unexpected scheme: %s", auth.Scheme)
	}
	if auth.Description != "Use a bearer token" {
		t.Fatalf("unexpected description: %s", auth.Description)
	}
}

func TestMergeExampleMergesMapsAndSlices(t *testing.T) {
	base := map[string]any{"a": "<string>", "b": "<number>"}
	example := map[string]any{"b": "42", "c": "extra"}

	merged := mergeExample(base, example).(map[string]any)
	if merged["b"] != "42" || merged["a"] != "<string>" || merged["c"] != "extra" {
		t.Fatalf("unexpected merged map: %#v", merged)
	}

	baseSlice := []any{"<string>"}
	exampleSlice := []any{"value"}
	mergedSlice := mergeExample(baseSlice, exampleSlice).([]any)
	if len(mergedSlice) != 1 || mergedSlice[0] != "value" {
		t.Fatalf("unexpected merged slice: %#v", mergedSlice)
	}
}

func TestFirstNonNullTypeSkipsNull(t *testing.T) {
	if got := firstNonNullType([]string{"null", "string"}); got != "string" {
		t.Fatalf("unexpected type: %s", got)
	}
}
