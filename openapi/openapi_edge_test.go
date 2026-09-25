package main

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/pb33f/libopenapi"
	"github.com/pb33f/libopenapi/datamodel/high/base"
	yaml "go.yaml.in/yaml/v4"
)

const bearerSpec = `openapi: 3.1.0
info:
  title: Auth
  version: 1.0.0
components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
security:
  - BearerAuth: []
paths: {}
`

func TestRunWithHashReadError(t *testing.T) {
	_, _, _, err := runWithHash("/does/not/exist.yaml", t.TempDir(), nil)
	if err == nil {
		t.Fatalf("expected read error")
	}
}

func TestRunSkipsWhenUnchanged(t *testing.T) {
	dir := t.TempDir()
	specPath := filepath.Join(dir, "spec.yaml")
	if err := os.WriteFile(specPath, []byte(sampleSpec), 0o644); err != nil {
		t.Fatalf("write spec: %v", err)
	}

	hash, changed, _, err := runWithHash(specPath, dir, nil)
	if err != nil {
		t.Fatalf("runWithHash: %v", err)
	}
	if !changed {
		t.Fatalf("expected change on first run")
	}

	_, changed, _, err = runWithHash(specPath, dir, &hash)
	if err != nil {
		t.Fatalf("runWithHash: %v", err)
	}
	if changed {
		t.Fatalf("expected unchanged on second run")
	}
}

func TestConvertParameterListEmpty(t *testing.T) {
	params, err := convertParameterList(nil)
	if err != nil {
		t.Fatalf("convert params: %v", err)
	}
	if params != nil {
		t.Fatalf("expected nil params")
	}
}

func TestConvertResponseNil(t *testing.T) {
	resp, err := convertResponse("200", nil)
	if err != nil {
		t.Fatalf("convert response: %v", err)
	}
	if resp != nil {
		t.Fatalf("expected nil response list")
	}
}

func TestBuildExampleForMediaTypeNil(t *testing.T) {
	val, err := buildExampleForMediaType(nil)
	if err != nil {
		t.Fatalf("build example: %v", err)
	}
	if val != nil {
		t.Fatalf("expected nil example")
	}
}

func TestLookupSecurityDescriptionDefaultsBearer(t *testing.T) {
	doc, err := libopenapi.NewDocument([]byte(bearerSpec))
	if err != nil {
		t.Fatalf("new doc: %v", err)
	}
	model, err := doc.BuildV3Model()
	if err != nil {
		t.Fatalf("build model: %v", err)
	}
	components := model.Model.Components

	auth := lookupSecurityDoc(components, "BearerAuth")
	if auth.Description != "Include an Authorization header with a Bearer token." {
		t.Fatalf("unexpected message: %s", auth.Description)
	}
	if auth.Type != "http" || auth.HttpScheme != "bearer" {
		t.Fatalf("unexpected auth metadata: %#v", auth)
	}
}

func TestYamlNodeToInterfaceNil(t *testing.T) {
	val, err := yamlNodeToInterface(nil)
	if err != nil {
		t.Fatalf("yaml node: %v", err)
	}
	if val != nil {
		t.Fatalf("expected nil value")
	}
}

func TestRenderSchemaHandlesObject(t *testing.T) {
	spec := loadSpec(t, schemaSpec)
	proxy := getSchemaProxy(t, spec, "ObjectSchema")
	val, err := renderSchema(proxy.Schema())
	if err != nil {
		t.Fatalf("render schema: %v", err)
	}
	if val == nil {
		t.Fatalf("expected rendered schema")
	}
}

func TestNormalizeYAMLArray(t *testing.T) {
	input := []interface{}{map[interface{}]interface{}{"k": "v"}}
	normalized := normalizeYAML(input).([]interface{})
	inner := normalized[0].(map[string]any)
	if inner["k"] != "v" {
		t.Fatalf("unexpected normalized: %#v", normalized)
	}
}

func TestSchemaProxyToInterfaceNil(t *testing.T) {
	val, err := schemaProxyToInterface(nil)
	if err != nil {
		t.Fatalf("schema proxy: %v", err)
	}
	if val != nil {
		t.Fatalf("expected nil value")
	}
}

func TestToStringAnyMapNil(t *testing.T) {
	if _, ok := toStringAnyMap(nil); ok {
		t.Fatalf("expected false")
	}
}

func TestToAnySliceNil(t *testing.T) {
	if _, ok := toAnySlice(nil); ok {
		t.Fatalf("expected false")
	}
}

func TestExtractExampleValueDataValue(t *testing.T) {
	dataNode := &yaml.Node{}
	if err := yaml.Unmarshal([]byte("data"), dataNode); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	value, err := extractExampleValue(&base.Example{DataValue: dataNode.Content[0]})
	if err != nil {
		t.Fatalf("extract example: %v", err)
	}
	if value != "data" {
		t.Fatalf("unexpected data example: %#v", value)
	}
}
