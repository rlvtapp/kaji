package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLocalReferenceClosureChangesBeforeCacheHit(t *testing.T) {
	root := t.TempDir()
	input := filepath.Join(root, "api.yaml")
	child := filepath.Join(root, "model.yaml")
	out := filepath.Join(root, "out")
	spec := `openapi: 3.2.0
info: {title: Local, version: "1"}
components:
  schemas:
    Item: {$ref: './model.yaml#/Item'}
paths:
  /item:
    get:
      operationId: getItem
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema: {$ref: '#/components/schemas/Item'}
`
	if err := os.WriteFile(input, []byte(spec), 0644); err != nil {
		t.Fatal(err)
	}
	write := func(kind string) {
		t.Helper()
		if err := os.WriteFile(child, []byte("Item:\n  type: object\n  properties:\n    value: {type: "+kind+"}\n"), 0644); err != nil {
			t.Fatal(err)
		}
	}
	write("string")
	first, changed, count, err := runWithHash(input, out, nil)
	if err != nil || !changed || count != 1 {
		t.Fatalf("firstcompile %v %v %d", err, changed, count)
	}
	catalog, err := os.ReadFile(filepath.Join(out, "schemas.json"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(catalog), `"Item"`) {
		t.Fatalf("external model omitted: %s", catalog)
	}
	_, changed, _, err = runWithHash(input, out, &first)
	if err != nil || changed {
		t.Fatalf("unchangedclosure incorrectly regenerated %v %v", changed, err)
	}
	write("integer")
	second, changed, _, err := runWithHash(input, out, &first)
	if err != nil || !changed || second == first {
		t.Fatalf("childchange missed %v %v", changed, err)
	}
	manifest, err := os.ReadFile(filepath.Join(out, "source.json"))
	if err != nil {
		t.Fatal(err)
	}
	var doc SourceClosureDoc
	if err := json.Unmarshal(manifest, &doc); err != nil {
		t.Fatal(err)
	}
	if len(doc.Files) != 2 || len(doc.SHA256) != 64 {
		t.Fatalf("invalidclosuremanifest %+v", doc)
	}
	if err := os.Remove(child); err != nil {
		t.Fatal(err)
	}
	_, _, _, err = runWithHash(input, out, &second)
	if err == nil || !strings.Contains(err.Error(), "local reference") {
		t.Fatalf("cachehidmissingchild: %v", err)
	}
}

func TestMutualRecursiveSchemasStayReferencesAndMissingRefsStillFail(t *testing.T) {
	for _, missing := range []bool{false, true} {
		t.Run(map[bool]string{false: "mutual", true: "missing"}[missing], func(t *testing.T) {
			root := t.TempDir()
			input := filepath.Join(root, "api.json")
			out := filepath.Join(root, "out")
			ref := "#/components/schemas/A"
			if missing {
				ref = "#/components/schemas/Missing"
			}
			spec := `{"openapi":"3.2.0","info":{"title":"Recursive","version":"1"},"paths":{"/item":{"get":{"responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"$ref":"#/components/schemas/A"}}}}}}}},"components":{"schemas":{"A":{"type":"object","required":["b"],"properties":{"b":{"$ref":"#/components/schemas/B"}}},"B":{"type":"object","required":["a"],"properties":{"a":{"$ref":"` + ref + `"}}}}}}`
			if err := os.WriteFile(input, []byte(spec), 0644); err != nil {
				t.Fatal(err)
			}
			_, _, _, err := runWithHash(input, out, nil)
			if missing {
				if err == nil {
					t.Fatal("acceptedmissingmodel")
				}
				if _, statErr := os.Stat(out); !os.IsNotExist(statErr) {
					t.Fatalf("wrotebeforebadref: %v", statErr)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			catalog, err := os.ReadFile(filepath.Join(out, "schemas.json"))
			if err != nil {
				t.Fatal(err)
			}
			for _, wanted := range []string{`#/components/schemas/A`, `#/components/schemas/B`, `"required"`} {
				if !strings.Contains(string(catalog), wanted) {
					t.Fatalf("recursiveIRloss %s", catalog)
				}
			}
		})
	}
}

func TestExternalRecursiveSchemasAndDiscriminatorMappingsNormalizeTogether(t *testing.T) {
	root := t.TempDir()
	input := filepath.Join(root, "api.yaml")
	out := filepath.Join(root, "out")
	spec := `openapi: 3.2.0
info: {title: Recursive, version: "1"}
components:
  schemas:
    Tree: {$ref: './nodes.yaml#/Tree'}
paths: {}
`
	child := `Tree:
  type: object
  properties:
    child: {$ref: '#/Tree'}
    example: {type: string}
  discriminator:
    propertyName: kind
    mapping: {tree: '#/Tree'}
`
	for path, data := range map[string]string{input: spec, filepath.Join(root, "nodes.yaml"): child} {
		if err := os.WriteFile(path, []byte(data), 0644); err != nil {
			t.Fatal(err)
		}
	}
	_, _, _, err := runWithHash(input, out, nil)
	if err != nil {
		t.Fatal(err)
	}
	bytes, err := os.ReadFile(filepath.Join(out, "schemas.json"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(bytes), "Local_Tree_") || strings.Contains(string(bytes), `"tree": "#/Tree"`) {
		t.Fatalf("lostrecursiveidentityormapping %s", bytes)
	}
}

func TestExternalNonSchemaCyclesAndBadFragmentsFailBeforeArtifacts(t *testing.T) {
	for _, ref := range []string{"./response.yaml", "./response.yaml#/missing"} {
		t.Run(ref, func(t *testing.T) {
			root := t.TempDir()
			input := filepath.Join(root, "api.yaml")
			out := filepath.Join(root, "out")
			spec := `openapi: 3.2.0
info: {title: Response, version: "1"}
paths:
  /item:
    get:
      responses:
        '200': {$ref: '` + ref + `'}
`
			if err := os.WriteFile(input, []byte(spec), 0644); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(filepath.Join(root, "response.yaml"), []byte("$ref: './response.yaml'\n"), 0644); err != nil {
				t.Fatal(err)
			}
			_, _, _, err := runWithHash(input, out, nil)
			if err == nil {
				t.Fatal("acceptednon-schema cycle/badfragment")
			}
			if _, stat := os.Stat(out); !os.IsNotExist(stat) {
				t.Fatalf("wroteartifacts: %v", stat)
			}
		})
	}
}

func TestNestedLocalSchemaTargetsBecomeNamedComponents(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "api.yaml")
	source := `openapi: 3.0.3
info: {title: Nested, version: '1'}
components:
  schemas:
    Invoice:
      type: object
      properties:
        currency: {$ref: '#/components/schemas/Invoice/definitions/Currency'}
      definitions:
        Currency:
          type: string
          enum: [USD, EUR]
paths:
  /invoice:
    get:
      operationId: getInvoice
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema: {$ref: '#/components/schemas/Invoice'}
`
	if err := os.WriteFile(path, []byte(source), 0600); err != nil {
		t.Fatal(err)
	}
	output := filepath.Join(root, "artifacts")
	if err := run(path, output); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(output, "schemas.json"))
	if err != nil {
		t.Fatal(err)
	}
	var catalog ComponentSchemasDoc
	if err := json.Unmarshal(data, &catalog); err != nil {
		t.Fatal(err)
	}
	names := map[string]bool{}
	for _, schema := range catalog.Schemas {
		names[schema.Name] = true
	}

	found := false
	for _, schema := range catalog.Schemas {
		if schema.Name != "Invoice" {
			continue
		}
		object := schema.Schema.(map[string]any)
		properties := object["properties"].(map[string]any)
		currency := properties["currency"].(map[string]any)
		target := strings.TrimPrefix(currency["$ref"].(string), "#/components/schemas/")
		if !names[target] {
			t.Fatalf("nested reference has no emitted model: %s", target)
		}
		found = true
	}
	if !found {
		t.Fatal("currency field missing")
	}
}

func TestVendorExtensionReferencesRemainLiteralAndAreNotFetched(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "api.yaml")
	source := `openapi: 3.0.3
info: {title: Extensions, version: '1'}
components:
  x-policy: {$ref: '../missing-policy.yaml'}
  schemas:
    x-named-model:
      type: object
      properties:
        child: {$ref: '#/components/schemas/Item'}
    Wrapper:
      type: object
      properties:
        x-named-field: {$ref: '#/components/schemas/x-named-model'}
    Item:
      type: string
      x-metadata: {$ref: '../missing-data.yaml'}
paths:
  /item:
    get:
      operationId: getItem
      x-poolster-custom: {$ref: '../custom-data.yaml'}
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema: {$ref: '#/components/schemas/Item'}
`
	if err := os.WriteFile(path, []byte(source), 0600); err != nil {
		t.Fatal(err)
	}
	closure, err := collectSourceClosure(path, []byte(source))
	if err != nil {
		t.Fatal(err)
	}
	if len(closure.Files) != 1 {
		t.Fatal("extension files entered source closure")
	}
	output := filepath.Join(root, "artifacts")
	if err := run(path, output); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(output, "schemas.json"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), `"$ref": "../missing-data.yaml"`) || strings.Contains(string(data), "x-poolster-opaque-reference") {
		t.Fatal("schema extension data was changed")
	}
}
