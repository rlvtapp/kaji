package main

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/netip"
	"strings"
	"testing"
)

func TestRemoteClosureBundlesRelativeRecursiveSchemasAndHashesChanges(t *testing.T) {
	root := []byte(`{"openapi":"3.2.0","info":{"title":"Remote","version":"1"},"paths":{},"components":{"schemas":{"Root":{"$ref":"https://schemas.example.test/root.yaml#/Root"}}}}`)
	docs := map[string]string{
		"https://schemas.example.test/root.yaml":  "Root:\n  type: object\n  properties:\n    child:\n      $ref: child.yaml#/Child\n",
		"https://schemas.example.test/child.yaml": "Child:\n  type: object\n  properties:\n    next:\n      $ref: '#/Child'\n    value:\n      type: string\n",
	}
	calls := map[string]int{}
	fetch := func(target string) ([]byte, error) { calls[target]++; return []byte(docs[target]), nil }
	source, err := collectSourceClosureWithFetcher("/tmp/root.json", root, fetch)
	if err != nil {
		t.Fatal(err)
	}
	if len(source.Manifest.Files) != 3 || calls["https://schemas.example.test/child.yaml"] != 1 {
		t.Fatalf("closure %+v calls %+v", source.Manifest, calls)
	}
	bundled, err := bundleLocalSources(source, "/tmp/root.json")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(bundled), "https://schemas.example.test") || !strings.Contains(string(bundled), "Local_Child_") {
		t.Fatalf("unresolved bundle: %s", bundled)
	}
	docs["https://schemas.example.test/child.yaml"] = strings.ReplaceAll(docs["https://schemas.example.test/child.yaml"], "type: string", "type: integer")
	changed, err := collectSourceClosureWithFetcher("/tmp/root.json", root, fetch)
	if err != nil {
		t.Fatal(err)
	}
	if source.Hash == changed.Hash || source.Manifest.SHA256 == changed.Manifest.SHA256 {
		t.Fatal("remote child changes must invalidate cache and source digest")
	}
}

func TestRemoteReferencePolicy(t *testing.T) {
	for _, ref := range []string{"http://example.com/a", "file:///etc/passwd", "https://user:password@example.com/a", "//example.com/a"} {
		if _, _, err := resolveSourceReference("/tmp/root.yaml", ref); err == nil {
			t.Fatalf("accepted %s", ref)
		}
	}
	target, fragment, err := resolveSourceReference("https://example.com/spec/root.yaml?version=1", "../models.yaml?v=2#/Widget")
	if err != nil || target != "https://example.com/models.yaml?v=2" || fragment != "/Widget" {
		t.Fatalf("%s %s %v", target, fragment, err)
	}
	for _, address := range []string{"127.0.0.1:443", "[::1]:443", "10.0.0.1:443", "169.254.169.254:443"} {
		if connection, err := publicReferenceDial(context.Background(), "tcp", address); err == nil {
			connection.Close()
			t.Fatalf("accepted internal address %s", address)
		}
	}
}

func TestRemoteFetchBoundedHTTPResponse(t *testing.T) {
	server := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/ok":
			fmt.Fprint(w, "type: string")
		case "/redirect":
			http.Redirect(w, r, "/ok", http.StatusFound)
		case "/large":
			fmt.Fprint(w, strings.Repeat("x", (16<<20)+1))
		default:
			w.WriteHeader(http.StatusForbidden)
		}
	}))
	defer server.Close()
	client := server.Client()
	client.CheckRedirect = func(req *http.Request, via []*http.Request) error { return fmt.Errorf("redirect disabled") }
	data, err := fetchRemoteSourceWithClient(server.URL+"/ok", client)
	if err != nil || string(data) != "type: string" {
		t.Fatalf("%s %v", data, err)
	}
	for _, path := range []string{"/redirect", "/large", "/missing"} {
		if _, err := fetchRemoteSourceWithClient(server.URL+path, client); err == nil {
			t.Fatalf("accepted %s", path)
		}
	}
}

func TestRemoteFetchRejectsReservedAddressRanges(t *testing.T) {
	for _, addr := range []string{"100.64.0.1", "192.0.2.1", "198.18.0.1", "203.0.113.1", "240.0.0.1", "64:ff9b::a00:1", "2002:a00:100::1", "::ffff:127.0.0.1"} {
		if publicReferenceAddress(netip.MustParseAddr(addr)) {
			t.Fatalf("accepted reserved %s", addr)
		}
	}
	for _, addr := range []string{"8.8.8.8", "2606:4700:4700::1111"} {
		if !publicReferenceAddress(netip.MustParseAddr(addr)) {
			t.Fatalf("rejected public %s", addr)
		}
	}
}

func TestDownloadedRemoteRootUsesOriginWithoutRefetchOrCredentials(t *testing.T) {
	root := []byte(`{"openapi":"3.2.0","info":{"title":"Remote","version":"1"},"paths":{},"components":{"schemas":{"Root":{"$ref":"models/child.yaml#/Child"}}}}`)
	var requested []string
	fetch := func(target string) ([]byte, error) {
		requested = append(requested, target)
		if target != "https://example.com/api/models/child.yaml" {
			return nil, fmt.Errorf("unexpected URL %s", target)
		}
		return []byte("Child:\n  type: string\n"), nil
	}
	source, err := collectSourceClosureWithOrigin("/tmp/downloaded.yaml", root, "https://example.com/api/root.yaml?root-token=private", fetch)
	if err != nil {
		t.Fatal(err)
	}
	if len(requested) != 1 {
		t.Fatalf("root should not be fetched again: %v", requested)
	}
	bundled, err := bundleLocalSources(source, "/tmp/downloaded.yaml")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(bundled), "models/child.yaml") {
		t.Fatalf("relative URL not bundled: %s", bundled)
	}
	for _, origin := range []string{"https://user:secret@example.com/root.yaml", "/tmp/root.yaml"} {
		if _, err := collectSourceClosureWithOrigin("/tmp/downloaded.yaml", root, origin, fetch); err == nil {
			t.Fatalf("accepted invalid source origin %s", origin)
		}
	}
}

func TestDownloadedHTTPRootDoesNotBreakOrFetchHTTPChildren(t *testing.T) {
	root := []byte(`{"openapi":"3.2.0","info":{"title":"Remote","version":"1"},"paths":{},"components":{"schemas":{"Root":{"type":"object","properties":{"next":{"$ref":"#/components/schemas/Root"}}}}}}`)
	fetch := func(target string) ([]byte, error) { t.Fatalf("unexpected fetch %s", target); return nil, nil }
	source, err := collectSourceClosureWithOrigin("/tmp/downloaded.yaml", root, "http://example.com/root.yaml", fetch)
	if err != nil {
		t.Fatal(err)
	}
	if source.Root != "http://example.com/root.yaml" {
		t.Fatal(source.Root)
	}
	if _, _, err := resolveSourceReference(source.Root, "child.yaml#/Child"); err == nil {
		t.Fatal("HTTP referenced child must be rejected")
	}
	if target, _, err := resolveSourceReference(source.Root, "#/components/schemas/Root"); err != nil || target != source.Root {
		t.Fatalf("root-local pointer %s %v", target, err)
	}
}
