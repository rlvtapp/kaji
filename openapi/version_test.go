package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestVersionBoundaryPreservesArtifacts(t *testing.T) {
	for _, version := range []string{"3.3.0", "4.0.0"} {
		t.Run(version, func(t *testing.T) {
			root := t.TempDir()
			spec := filepath.Join(root, "spec.json")
			out := filepath.Join(root, "artifacts")
			if err := os.Mkdir(out, 0755); err != nil {
				t.Fatal(err)
			}
			marker := filepath.Join(out, "operations.json")
			if err := os.WriteFile(marker, []byte("customer-owned"), 0644); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(spec, []byte(`{"openapi":"`+version+`","info":{"title":"Future","version":"1"},"paths":{"/items":{"query":{"operationId":"searchItems"}}}}`), 0644); err != nil {
				t.Fatal(err)
			}
			_, _, _, err := runWithHash(spec, out, nil)
			if err == nil || !strings.Contains(err.Error(), "unsupported OpenAPI version") {
				t.Fatalf("expected precise capability diagnostic, got %v", err)
			}
			bytes, err := os.ReadFile(marker)
			if err != nil || string(bytes) != "customer-owned" {
				t.Fatalf("modified artifacts: %q, %v", bytes, err)
			}
		})
	}
}

func TestSupportedDocumentVersions(t *testing.T) {
	for _, input := range []string{`{"openapi":"3.0.4"}`, `{"openapi":"3.1.2"}`, "openapi: 3.1.0\n", "openapi: 3.2.0\n", "openapi: 3.2.1\n", "swagger: '2.0'\n"} {
		if err := validateDocumentVersion([]byte(input)); err != nil {
			t.Fatalf("%s: %v", input, err)
		}
	}
	for _, input := range []string{"openapi: 3.3.0\n", "openapi: 3.1evil\n", "info: {}\n"} {
		if err := validateDocumentVersion([]byte(input)); err == nil {
			t.Fatalf("accepted %s", input)
		}
	}
}
