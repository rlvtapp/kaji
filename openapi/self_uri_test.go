package main

import (
	"strings"
	"testing"
)

func TestSelfURIControlsRelativeReferencesWithoutRefetchingRoot(t *testing.T) {
	root := []byte(`openapi: 3.2.0
$self: https://api.example.com/contracts/root.yaml
info: {title: Self, version: '1'}
paths: {}
components:
  schemas:
    Item:
      $ref: models.yaml#/Item
    Self:
      $ref: https://api.example.com/contracts/root.yaml#/components/schemas/Item
`)
	var fetched []string
	closure, err := collectSourceClosureWithFetcher("/tmp/downloaded/spec.yaml", root, func(uri string) ([]byte, error) {
		fetched = append(fetched, uri)
		if uri != "https://api.example.com/contracts/models.yaml" {
			t.Fatalf("unexpected fetch %s", uri)
		}
		return []byte("Item: {type: object, properties: {id: {type: string}}}"), nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(fetched) != 1 || len(closure.Documents) != 2 {
		t.Fatalf("unexpected source closure: %v", fetched)
	}
	bundled, err := bundleLocalSources(closure, "/tmp/downloaded/spec.yaml")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(bundled), "models.yaml#/Item") {
		t.Fatalf("unresolved relative reference: %s", bundled)
	}
}

func TestSelfURIRejectsCredentialsAndFragments(t *testing.T) {
	for _, identity := range []string{"https://user:secret@example.com/api.yaml", "https://example.com/api.yaml#schema"} {
		_, err := collectSourceClosureWithFetcher("/tmp/spec.yaml", []byte("openapi: 3.2.0\n$self: "+identity+"\npaths: {}"), func(string) ([]byte, error) { t.Fatal("invalid identity fetched"); return nil, nil })
		if err == nil {
			t.Fatalf("accepted invalid identity %s", identity)
		}
	}
}
