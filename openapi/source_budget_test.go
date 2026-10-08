package main

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSourceClosureSeparatesRootAndReferenceBudgets(t *testing.T) {
	path := filepath.Join(t.TempDir(), "root.yaml")
	large := append([]byte("openapi: 3.1.0\npaths: {}\n#"), bytes.Repeat([]byte("x"), 17<<20)...)
	if _, err := collectSourceClosure(path, large); err != nil {
		t.Fatalf("large root contract rejected: %v", err)
	}
	if _, err := collectSourceClosure(path, make([]byte, 65<<20)); err == nil || !strings.Contains(err.Error(), "64 MiB") {
		t.Fatalf("root budget not enforced: %v", err)
	}
	reference := filepath.Join(filepath.Dir(path), "child.yaml")
	if err := os.WriteFile(reference, large, 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := collectSourceClosure(path, []byte("openapi: 3.1.0\npaths: {}\ncomponents:\n  schemas:\n    Child:\n      $ref: child.yaml\n")); err == nil || !strings.Contains(err.Error(), "16 MiB") {
		t.Fatalf("reference budget not enforced: %v", err)
	}
}
