package main

import (
	"testing"

	"github.com/pb33f/libopenapi"
	"github.com/pb33f/libopenapi/datamodel/high/base"
	v3 "github.com/pb33f/libopenapi/datamodel/high/v3"
	"github.com/pb33f/libopenapi/orderedmap"
	yaml "go.yaml.in/yaml/v4"
)

const schemaSpec = `openapi: 3.1.0
info:
  title: Sample
  version: 1.0.0
components:
  schemas:
    ConstSchema:
      type: string
      const: fixed
    DefaultSchema:
      type: integer
      default: 7
    EnumSchema:
      type: string
      enum: [alpha, beta]
    OneOfSchema:
      oneOf:
        - type: string
        - type: integer
    AnyOfSchema:
      anyOf:
        - type: number
        - type: integer
    ArraySchema:
      type: array
      items:
        type: string
    ObjectSchema:
      type: object
      properties:
        name:
          type: string
        count:
          type: integer
    AllOfSchema:
      allOf:
        - type: object
          properties:
            a:
              type: string
        - type: object
          properties:
            b:
              type: number
    RefSchema:
      $ref: '#/components/schemas/ObjectSchema'
    FormatSchema:
      type: string
      format: date-time
    EnumNumberSchema:
      type: integer
      enum: [1, 2]
paths: {}
`

const exampleSpec = `openapi: 3.1.0
info:
  title: Example
  version: 1.0.0
paths:
  /example:
    get:
      responses:
        '200':
          description: OK
          content:
            application/json:
              example:
                foo: bar
              examples:
                default:
                  value:
                    foo: baz
              schema:
                type: object
                properties:
                  foo:
                    type: string
`

func loadSpec(t *testing.T, spec string) *v3.Document {
	t.Helper()

	doc, err := libopenapi.NewDocument([]byte(spec))
	if err != nil {
		t.Fatalf("new document: %v", err)
	}

	model, err := doc.BuildV3Model()
	if err != nil {
		t.Fatalf("build model: %v", err)
	}

	docModel := model.Model
	return &docModel
}

func getSchemaProxy(t *testing.T, spec *v3.Document, name string) *base.SchemaProxy {
	t.Helper()

	if spec.Components == nil || spec.Components.Schemas == nil {
		t.Fatalf("schemas missing")
	}

	for pair := spec.Components.Schemas.First(); pair != nil; pair = pair.Next() {
		if pair.Key() == name {
			return pair.Value()
		}
	}

	t.Fatalf("schema %s not found", name)
	return nil
}

func TestGenerateExampleFromSchemaProxyVariants(t *testing.T) {
	spec := loadSpec(t, schemaSpec)

	cases := map[string]any{
		"ConstSchema":   "fixed",
		"DefaultSchema": 7,
		"EnumSchema":    "alpha",
		"OneOfSchema":   "<string>",
		"AnyOfSchema":   "<number>",
	}

	for name, expected := range cases {
		proxy := getSchemaProxy(t, spec, name)
		value, err := generateExampleFromSchemaProxy(proxy)
		if err != nil {
			t.Fatalf("%s: %v", name, err)
		}
		if value != expected {
			t.Fatalf("%s: expected %v, got %v", name, expected, value)
		}
	}

	arrayProxy := getSchemaProxy(t, spec, "ArraySchema")
	arrayValue, err := generateExampleFromSchemaProxy(arrayProxy)
	if err != nil {
		t.Fatalf("array schema: %v", err)
	}
	slice, ok := arrayValue.([]any)
	if !ok || len(slice) == 0 || slice[0] != "<string>" {
		t.Fatalf("unexpected array example: %#v", arrayValue)
	}

	objProxy := getSchemaProxy(t, spec, "ObjectSchema")
	objValue, err := generateExampleFromSchemaProxy(objProxy)
	if err != nil {
		t.Fatalf("object schema: %v", err)
	}
	objMap, ok := objValue.(map[string]any)
	if !ok || objMap["name"] != "<string>" || objMap["count"] != "<integer>" {
		t.Fatalf("unexpected object example: %#v", objValue)
	}

	allOfProxy := getSchemaProxy(t, spec, "AllOfSchema")
	allOfValue, err := generateExampleFromSchemaProxy(allOfProxy)
	if err != nil {
		t.Fatalf("allOf schema: %v", err)
	}
	allOfMap, ok := allOfValue.(map[string]any)
	if !ok || allOfMap["a"] != "<string>" || allOfMap["b"] != "<number>" {
		t.Fatalf("unexpected allOf example: %#v", allOfValue)
	}
}

func TestSchemaProxyToInterfaceIncludesReference(t *testing.T) {
	spec := loadSpec(t, schemaSpec)
	proxy := getSchemaProxy(t, spec, "RefSchema")

	value, err := schemaProxyToInterface(proxy)
	if err != nil {
		t.Fatalf("schema proxy: %v", err)
	}
	result, ok := value.(map[string]any)
	if !ok {
		t.Fatalf("expected map, got %#v", value)
	}
	if result["$ref"] == nil {
		t.Fatalf("expected $ref in result: %#v", result)
	}
}

func TestSchemaHelpers(t *testing.T) {
	spec := loadSpec(t, schemaSpec)

	objectProxy := getSchemaProxy(t, spec, "ObjectSchema")
	schema := objectProxy.Schema()
	fields := schemaToFields(schema)
	if len(fields) != 2 {
		t.Fatalf("expected 2 fields, got %d", len(fields))
	}

	formatProxy := getSchemaProxy(t, spec, "FormatSchema")
	if got := summarizeSchemaType(formatProxy.Schema()); got != "string:date-time" {
		t.Fatalf("unexpected format type: %s", got)
	}

	enumProxy := getSchemaProxy(t, spec, "EnumNumberSchema")
	if got := summarizeSchemaType(enumProxy.Schema()); got != "enum<integer>" {
		t.Fatalf("unexpected enum type: %s", got)
	}
	values := enumValues(enumProxy.Schema())
	if len(values) != 2 || values[0] != "1" || values[1] != "2" {
		t.Fatalf("unexpected enum values: %#v", values)
	}
}

func TestExtractExampleValueAndMergeHelpers(t *testing.T) {
	node := &yaml.Node{}
	if err := yaml.Unmarshal([]byte("value"), node); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	example := &base.Example{Value: node.Content[0]}
	value, err := extractExampleValue(example)
	if err != nil {
		t.Fatalf("extract example: %v", err)
	}
	if value != "value" {
		t.Fatalf("unexpected example: %#v", value)
	}

	serialized := &base.Example{SerializedValue: "raw"}
	value, err = extractExampleValue(serialized)
	if err != nil {
		t.Fatalf("extract serialized: %v", err)
	}
	if value != "raw" {
		t.Fatalf("unexpected serialized: %#v", value)
	}

	external := &base.Example{ExternalValue: "https://example.com"}
	value, err = extractExampleValue(external)
	if err != nil {
		t.Fatalf("extract external: %v", err)
	}
	if value != "https://example.com" {
		t.Fatalf("unexpected external: %#v", value)
	}

	if _, ok := toStringAnyMap(map[string]interface{}{"a": 1}); !ok {
		t.Fatalf("expected map conversion")
	}
	if _, ok := toAnySlice([]interface{}{"a"}); !ok {
		t.Fatalf("expected slice conversion")
	}
}

func TestBuildExampleForMediaTypeUsesExample(t *testing.T) {
	spec := loadSpec(t, exampleSpec)
	_, op := getOperation(t, spec, "get")
	response := op.Responses.Codes.First().Value()
	mediaType := response.Content.First().Value()

	value, err := buildExampleForMediaType(mediaType)
	if err != nil {
		t.Fatalf("build example: %v", err)
	}

	result, ok := value.(map[string]any)
	if !ok || result["foo"] != "bar" {
		t.Fatalf("unexpected example value: %#v", value)
	}
}

func TestNormalizeYAMLHandlesInterfaceMap(t *testing.T) {
	input := map[interface{}]interface{}{"k": "v", 1: "n"}
	normalized := normalizeYAML(input).(map[string]any)
	if normalized["k"] != "v" || normalized["1"] != "n" {
		t.Fatalf("unexpected normalized: %#v", normalized)
	}
}

func TestSchemaFieldsFromMediaTypeNil(t *testing.T) {
	fields, err := schemaFieldsFromMediaType(nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if fields != nil {
		t.Fatalf("expected nil fields")
	}
}

func TestBuildExampleForMediaTypeExamplesFallback(t *testing.T) {
	mediaType := &v3.MediaType{Examples: orderedmap.New[string, *base.Example]()}
	exampleNode := &yaml.Node{}
	if err := yaml.Unmarshal([]byte("example"), exampleNode); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	mediaType.Examples.Set("default", &base.Example{Value: exampleNode.Content[0]})

	value, err := buildExampleForMediaType(mediaType)
	if err != nil {
		t.Fatalf("build example: %v", err)
	}
	if value != "example" {
		t.Fatalf("unexpected value: %#v", value)
	}
}
