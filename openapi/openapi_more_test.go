package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/pb33f/libopenapi/datamodel/high/base"
	v3 "github.com/pb33f/libopenapi/datamodel/high/v3"
	"github.com/pb33f/libopenapi/orderedmap"
	yaml "go.yaml.in/yaml/v4"
)

func TestConvertResponsesHandlesDefaultOnly(t *testing.T) {
	response := &v3.Response{Description: "default response"}
	responses := &v3.Responses{Default: response}

	converted, err := convertResponses(responses)
	if err != nil {
		t.Fatalf("convert responses: %v", err)
	}
	if len(converted) != 1 {
		t.Fatalf("expected 1 response, got %d", len(converted))
	}
	if converted[0].Code != "default" {
		t.Fatalf("unexpected response code: %s", converted[0].Code)
	}
}

func TestSchemaFieldsFromMediaTypeUsesSchema(t *testing.T) {
	spec := loadSampleSpec(t, sampleSpec)
	_, op := getOperation(t, spec, "get")
	response := op.Responses.Codes.First().Value()
	mediaType := response.Content.First().Value()

	fields, err := schemaFieldsFromMediaType(mediaType)
	if err != nil {
		t.Fatalf("schema fields: %v", err)
	}
	if len(fields) != 2 {
		t.Fatalf("expected 2 fields, got %d", len(fields))
	}

	var itemsField *SchemaField
	var totalField *SchemaField
	for i := range fields {
		if fields[i].Name == "items" {
			itemsField = &fields[i]
		}
		if fields[i].Name == "total" {
			totalField = &fields[i]
		}
	}
	if itemsField == nil || totalField == nil {
		t.Fatalf("missing fields: %#v", fields)
	}
	if len(itemsField.Children) < 2 {
		t.Fatalf("expected nested fields for items")
	}
}

func TestBuildExampleForMediaTypeFallbackUsesFirstExample(t *testing.T) {
	mediaType := &v3.MediaType{Examples: orderedmap.New[string, *base.Example]()}

	firstNode := &yaml.Node{}
	if err := yaml.Unmarshal([]byte("first"), firstNode); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	secondNode := &yaml.Node{}
	if err := yaml.Unmarshal([]byte("second"), secondNode); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	mediaType.Examples.Set("first", &base.Example{Value: firstNode.Content[0]})
	mediaType.Examples.Set("second", &base.Example{Value: secondNode.Content[0]})

	value, err := buildExampleForMediaType(mediaType)
	if err != nil {
		t.Fatalf("build example: %v", err)
	}
	if value != "first" {
		t.Fatalf("unexpected example value: %#v", value)
	}
}

func TestWriteJSONRejectsNonJSONExtension(t *testing.T) {
	path := filepath.Join(t.TempDir(), "output.txt")
	if err := writeJSON(path, map[string]any{"ok": true}); err == nil {
		t.Fatalf("expected error for non-json path")
	}
}

func TestWriteJSONWritesFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "nested", "output.json")
	if err := writeJSON(path, map[string]any{"ok": true}); err != nil {
		t.Fatalf("write json: %v", err)
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("expected file to exist: %v", err)
	}
}

func TestRunWithHashReportsInvalidSpec(t *testing.T) {
	path := filepath.Join(t.TempDir(), "spec.yaml")
	if err := os.WriteFile(path, []byte("openapi: ["), 0o644); err != nil {
		t.Fatalf("write spec: %v", err)
	}

	_, _, _, err := runWithHash(path, filepath.Join(t.TempDir(), "out"), nil)
	if err == nil {
		t.Fatalf("expected error for invalid spec")
	}
	if !strings.Contains(err.Error(), "parse spec") && !strings.Contains(err.Error(), "build v3 model") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestRunWithHashPreservesMintAndRlvtExtensions(t *testing.T) {
	spec := `openapi: 3.1.0
info:
  title: Extension API
  version: 1.0.0
paths:
  /users:
    get:
      summary: Original summary
      deprecated: true
      x-hidden: true
      x-mint:
        metadata:
          title: Mint users
        content: "Mint content"
      x-rlvt:
        title: Relevate users
        content: "Relevate content"
        sections:
          playground: false
      x-kaji-mock:
        scenarios:
          - name: rate-limited
            when:
              headers:
                x-test-scenario: rate-limited
            response:
              status: 429
              headers:
                retry-after: "1"
              body:
                message: Too many requests
      x-kaji-pagination:
        type: cursor
        inputs:
          cursor: cursor
      responses:
        '200':
          description: OK
`
	dir := t.TempDir()
	specPath := filepath.Join(dir, "spec.yaml")
	outDir := filepath.Join(dir, "out")
	if err := os.WriteFile(specPath, []byte(spec), 0o644); err != nil {
		t.Fatalf("write spec: %v", err)
	}

	if _, _, _, err := runWithHash(specPath, outDir, nil); err != nil {
		t.Fatalf("run with hash: %v", err)
	}

	data, err := os.ReadFile(filepath.Join(outDir, "operations", "get_users.json"))
	if err != nil {
		t.Fatalf("read operation: %v", err)
	}
	var operation OperationDoc
	if err := json.Unmarshal(data, &operation); err != nil {
		t.Fatalf("decode operation: %v", err)
	}
	if operation.Extensions["x-mint"] == nil || operation.Extensions["x-rlvt"] == nil || operation.Extensions["x-kaji-mock"] == nil || operation.Extensions["x-kaji-pagination"] == nil {
		t.Fatalf("expected docs and Kaji extensions, got %#v", operation.Extensions)
	}
	mock := operation.Extensions["x-kaji-mock"].(map[string]any)
	scenarios := mock["scenarios"].([]any)
	if scenarios[0].(map[string]any)["name"] != "rate-limited" {
		t.Fatalf("expected x-kaji-mock scenario to survive conversion, got %#v", mock)
	}
	if !operation.Deprecated {
		t.Fatalf("expected deprecated flag to be preserved")
	}
	if !operation.Hidden {
		t.Fatalf("expected x-hidden flag to be preserved")
	}
}

func TestResolveServersPrefersOperationThenPathThenRoot(t *testing.T) {
	root := []*v3.Server{{URL: "https://root.example.com"}}
	path := []*v3.Server{{URL: "https://path.example.com"}}
	op := []*v3.Server{{URL: "https://op.example.com"}}

	got := resolveServers(op, path, root)
	if len(got) != 1 || got[0].URL != "https://op.example.com" {
		t.Fatalf("expected operation server, got %#v", got)
	}

	got = resolveServers(nil, path, root)
	if len(got) != 1 || got[0].URL != "https://path.example.com" {
		t.Fatalf("expected path server, got %#v", got)
	}

	got = resolveServers(nil, nil, root)
	if len(got) != 1 || got[0].URL != "https://root.example.com" {
		t.Fatalf("expected root server, got %#v", got)
	}
}

func TestConvertServersIncludesVariables(t *testing.T) {
	variables := orderedmap.New[string, *v3.ServerVariable]()
	variables.Set("region", &v3.ServerVariable{
		Default:     "us",
		Enum:        []string{"us", "eu"},
		Description: "Deployment region",
	})
	serverDocs := convertServers([]*v3.Server{{
		Name:        "Production",
		URL:         "https://{region}.api.example.com",
		Description: "Main API",
		Variables:   variables,
	}})

	if len(serverDocs) != 1 {
		t.Fatalf("expected one server doc, got %d", len(serverDocs))
	}
	if serverDocs[0].Name != "Production" {
		t.Fatalf("unexpected server name: %s", serverDocs[0].Name)
	}
	if len(serverDocs[0].Variables) != 1 {
		t.Fatalf("expected one server variable, got %#v", serverDocs[0].Variables)
	}
	if serverDocs[0].Variables[0].Name != "region" || serverDocs[0].Variables[0].Default != "us" {
		t.Fatalf("unexpected server variable: %#v", serverDocs[0].Variables[0])
	}
}
