package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestOpenAPI32QueryAndNullableWebhook(t *testing.T) {
	for _, version := range []string{"3.2.0", "3.2.1"} {
		t.Run(version, func(t *testing.T) {
			root := t.TempDir()
			input := filepath.Join(root, "api.json")
			out := filepath.Join(root, "out")
			spec := `{"openapi":"` + version + `","info":{"title":"Query","version":"1"},"paths":{"/search":{"query":{"operationId":"search","requestBody":{"content":{"application/json":{"schema":{"type":"object","properties":{"term":{"type":["string","null"]}}}}}},"responses":{"200":{"description":"ok"}}}}},"webhooks":{"event":{"post":{"operationId":"event","responses":{"200":{"description":"ok"}}}}}}`
			if err := os.WriteFile(input, []byte(spec), 0644); err != nil {
				t.Fatal(err)
			}
			_, _, count, err := runWithHash(input, out, nil)
			if err != nil || count != 2 {
				t.Fatalf("count=%d err=%v", count, err)
			}
			files, err := os.ReadDir(filepath.Join(out, "operations"))
			if err != nil {
				t.Fatal(err)
			}
			found := false
			for _, file := range files {
				bytes, err := os.ReadFile(filepath.Join(out, "operations", file.Name()))
				if err != nil {
					t.Fatal(err)
				}
				if strings.Contains(string(bytes), `"QUERY"`) {
					found = true
					if !strings.Contains(string(bytes), `"null"`) {
						t.Fatalf("lost nullable body %s", bytes)
					}
				}
			}
			if !found {
				t.Fatal("QUERY omitted")
			}
		})
	}
}
func TestOpenAPI32UnsupportedWireFeaturesPreserveArtifacts(t *testing.T) {
	for _, tc := range []struct{ name, operation, diagnostic string }{
		{"querystring", `"get":{"parameters":[{"name":"whole","in":"querystring","content":{"application/json":{"schema":{"type":"object"}}}}],"responses":{"200":{"description":"ok"}}}`, "querystring"},
		{"itemSchema", `"get":{"responses":{"200":{"description":"ok","content":{"text/event-stream":{"itemSchema":{"type":"string"}}}}}}`, "itemSchema"},
		{"prefixEncoding", `"post":{"requestBody":{"content":{"multipart/mixed":{"schema":{"type":"array"},"prefixEncoding":[{}]}}},"responses":{"200":{"description":"ok"}}}`, "prefixEncoding"},
		{"custommethod", `"additionalOperations":{"COPY":{"responses":{"200":{"description":"ok"}}}}`, "additionalOperations"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			input := filepath.Join(root, "api.json")
			out := filepath.Join(root, "out")
			if err := os.MkdirAll(filepath.Join(out, "operations"), 0755); err != nil {
				t.Fatal(err)
			}
			marker := filepath.Join(out, "operations", "owned.json")
			if err := os.WriteFile(marker, []byte("owned"), 0644); err != nil {
				t.Fatal(err)
			}
			spec := `{"openapi":"3.2.0","info":{"title":"Unsupported","version":"1"},"paths":{"/items":{` + tc.operation + `}}}`
			if err := os.WriteFile(input, []byte(spec), 0644); err != nil {
				t.Fatal(err)
			}
			_, _, _, err := runWithHash(input, out, nil)
			if err == nil || !strings.Contains(err.Error(), tc.diagnostic) {
				t.Fatalf("want %s got %v", tc.diagnostic, err)
			}
			bytes, err := os.ReadFile(marker)
			if err != nil || string(bytes) != "owned" {
				t.Fatalf("artifact overwritten %q %v", bytes, err)
			}
		})
	}
}

func TestOpenAPI32ReferencedStreamingFailsBeforeWrite(t *testing.T) {
	root := t.TempDir()
	input := filepath.Join(root, "api.json")
	out := filepath.Join(root, "out")
	spec := `{"openapi":"3.2.0","info":{"title":"Refs","version":"1"},"paths":{"/events":{"get":{"responses":{"200":{"$ref":"#/components/responses/Events"}}}}},"components":{"responses":{"Events":{"description":"events","content":{"text/event-stream":{"itemSchema":{"type":"string"}}}}}}}`
	if err := os.WriteFile(input, []byte(spec), 0644); err != nil {
		t.Fatal(err)
	}
	_, _, _, err := runWithHash(input, out, nil)
	if err == nil || !strings.Contains(err.Error(), "itemSchema") {
		t.Fatalf("resolvedstreaming metadata lost: %v", err)
	}
	if _, err := os.Stat(out); !os.IsNotExist(err) {
		t.Fatalf("createdoutput before capability rejection: %v", err)
	}
}
func TestOpenAPI32WebhookOnlyAndSchemaPropertyNames(t *testing.T) {
	root := t.TempDir()
	input := filepath.Join(root, "api.json")
	out := filepath.Join(root, "out")
	spec := `{"openapi":"3.2.0","info":{"title":"Webhook","version":"1"},"webhooks":{"event":{"post":{"operationId":"event","requestBody":{"content":{"application/json":{"schema":{"type":"object","properties":{"itemSchema":{"type":"string"},"prefixEncoding":{"type":"string"}}}}}},"responses":{"200":{"description":"ok"}}}}}}`
	if err := os.WriteFile(input, []byte(spec), 0644); err != nil {
		t.Fatal(err)
	}
	_, _, count, err := runWithHash(input, out, nil)
	if err != nil || count != 1 {
		t.Fatalf("webhook-onlycontract count=%d err=%v", count, err)
	}
}

func TestOpenAPI32UnresolvedExternalReferencePreservesArtifacts(t *testing.T) {
	root := t.TempDir()
	input := filepath.Join(root, "api.json")
	out := filepath.Join(root, "out")
	if err := os.Mkdir(out, 0755); err != nil {
		t.Fatal(err)
	}
	marker := filepath.Join(out, "operations.json")
	if err := os.WriteFile(marker, []byte("owned"), 0644); err != nil {
		t.Fatal(err)
	}
	spec := `{"openapi":"3.2.0","info":{"title":"External","version":"1"},"paths":{"/items":{"get":{"responses":{"200":{"$ref":"./missing.yaml#/Response"}}}}}}`
	if err := os.WriteFile(input, []byte(spec), 0644); err != nil {
		t.Fatal(err)
	}
	_, _, _, err := runWithHash(input, out, nil)
	if err == nil {
		t.Fatal("accepted unresolved external response")
	}
	bytes, err := os.ReadFile(marker)
	if err != nil || string(bytes) != "owned" {
		t.Fatalf("artifact overwritten %q %v", bytes, err)
	}
}
