package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCompatibleYAMLPreservesBlockTabsAndUnicodeSeparators(t *testing.T) {
	for _, description := range []string{"description: |-\n  \t\n  content\n", "description: |\n  before\u2028after\u2029end\n"} {
		data := []byte("openapi: 3.0.3\ninfo:\n  title: Compatibility\n  version: '1'\n" + description + "paths: {}\n")
		dir := t.TempDir()
		path := filepath.Join(dir, "api.yaml")
		if err := os.WriteFile(path, data, 0600); err != nil {
			t.Fatal(err)
		}
		normalized, err := compatibleYAML(data)
		if err != nil {
			t.Fatal(err)
		}
		var document map[string]any
		if err := unmarshalDocument(normalized, &document); err != nil {
			t.Fatal(err)
		}
		value := document["description"].(string)
		if strings.Contains(description, "\t") && !strings.Contains(value, "\t") {
			t.Fatal("tab was changed")
		}
		if strings.Contains(description, "\u2028") && (!strings.Contains(value, "\u2028") || !strings.Contains(value, "\u2029")) {
			t.Fatal("Unicode content was changed")
		}
		closure, err := collectSourceClosure(path, data)
		if err != nil {
			t.Fatal(err)
		}
		if string(closure.Documents[closure.Root]) != string(data) {
			t.Fatal("provenance bytes changed")
		}
		encoded, _ := json.Marshal(document)
		if err := validateDocumentVersion(encoded); err != nil {
			t.Fatal(err)
		}
		if err := run(path, filepath.Join(dir, "artifacts")); err != nil {
			t.Fatal(err)
		}
	}
}
func TestCompatibleYAMLRejectsBrokenIndentation(t *testing.T) {
	if _, err := compatibleYAML([]byte("paths:\n \t/x:\n   get: {}\n")); err == nil {
		t.Fatal("invalid indentation accepted")
	}
}
