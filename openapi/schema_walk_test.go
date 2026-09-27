package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"unicode/utf8"

	"github.com/pb33f/libopenapi/datamodel/high/base"
	"github.com/pb33f/libopenapi/orderedmap"
)

func TestSchemaPreviewsStopRecursiveObjectsAndArrays(t *testing.T) {
	object := &base.Schema{Type: []string{"object"}, Properties: orderedmap.New[string, *base.SchemaProxy]()}
	object.Properties.Set("self", base.CreateSchemaProxy(object))
	object.Properties.Set("name", base.CreateSchemaProxy(&base.Schema{Type: []string{"string"}}))
	example, err := generateExampleFromSchema(object)
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := json.Marshal(example)
	if err != nil || len(encoded) > 200 {
		t.Fatalf("unbounded example: %s, %v", encoded, err)
	}
	fields := schemaToFields(object)
	if len(fields) != 2 || len(fields[0].Children) != 0 {
		t.Fatalf("recursive fields: %+v", fields)
	}
	array := &base.Schema{Type: []string{"array"}}
	array.Items = &base.DynamicValue[*base.SchemaProxy, bool]{A: base.CreateSchemaProxy(array)}
	if got := summarizeSchemaType(array); got != "any[]" {
		t.Fatalf("recursive type: %s", got)
	}
	if _, err := generateExampleFromSchema(array); err != nil {
		t.Fatal(err)
	}
	if got := schemaToFields(array); len(got) != 0 {
		t.Fatalf("recursive array: %+v", got)
	}
}

func TestSchemaPreviewsBoundWideGraphsAndKeepIndependentBranches(t *testing.T) {
	leaf := &base.Schema{Type: []string{"object"}, Properties: orderedmap.New[string, *base.SchemaProxy]()}
	leaf.Properties.Set("name", base.CreateSchemaProxy(&base.Schema{Type: []string{"string"}}))
	root := &base.Schema{Type: []string{"object"}, Properties: orderedmap.New[string, *base.SchemaProxy]()}
	root.Properties.Set("left", base.CreateSchemaProxy(leaf))
	root.Properties.Set("right", base.CreateSchemaProxy(leaf))
	example, err := generateExampleFromSchema(root)
	if err != nil {
		t.Fatal(err)
	}
	value := example.(map[string]any)
	if fmt.Sprint(value["left"]) != fmt.Sprint(value["right"]) {
		t.Fatalf("global visited set lost a branch: %+v", value)
	}
	for i := 0; i < schemaPreviewNodes*2; i++ {
		root.Properties.Set(fmt.Sprint(i), base.CreateSchemaProxy(leaf))
	}
	walk := newSchemaWalk()
	if _, err := walk.example(root); err != nil {
		t.Fatal(err)
	}
	if walk.remaining < 0 || walk.depth != 0 || len(walk.active) != 0 {
		t.Fatalf("invalid budget/state: %+v", walk)
	}
}

func TestLongOperationFilenamesAreBoundedAndDistinct(t *testing.T) {
	path := "/" + strings.Repeat("世界/longGraphResource/", 40)
	first, second := slugFor(path+"a", "get"), slugFor(path+"b", "get")
	if len(first) > 197 || !utf8.ValidString(first) || first == second || first != slugFor(path+"a", "get") {
		t.Fatalf("invalid slugs: %q %q", first, second)
	}
}

func TestNormalizedPathsDoNotOverwriteOperationArtifacts(t *testing.T) {
	spec := `openapi: 3.1.0
info: {title: Collisions, version: 1.0.0}
paths:
  /a-b:
    get:
      operationId: first
      responses: {'204': {description: OK}}
  /a_b:
    get:
      operationId: second
      responses: {'204': {description: OK}}
`
	dir := t.TempDir()
	input := filepath.Join(dir, "spec.yaml")
	if err := os.WriteFile(input, []byte(spec), 0600); err != nil {
		t.Fatal(err)
	}
	out := filepath.Join(dir, "output")
	if err := run(input, out); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(out, "operations.json"))
	if err != nil {
		t.Fatal(err)
	}
	var index map[string]string
	if err := json.Unmarshal(data, &index); err != nil {
		t.Fatal(err)
	}
	if len(index) != 2 || index["GET /a-b"] == index["GET /a_b"] {
		t.Fatalf("overwritten artifact: %v", index)
	}
}
