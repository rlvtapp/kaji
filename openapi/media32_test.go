package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestOpenAPI32WireMetadata(t *testing.T) {
	root := t.TempDir()
	input := filepath.Join(root, "api.json")
	out := filepath.Join(root, "out")
	spec := `{"openapi":"3.2.1","info":{"title":"Wire","version":"1"},"tags":[{"name":"events","summary":"Events","parent":"api","kind":"resource"}],"paths":{"/search":{"get":{"operationId":"search","parameters":[{"name":"filter","in":"querystring","content":{"application/x-www-form-urlencoded":{"schema":{"type":"object","properties":{"term":{"type":"string"}}},"encoding":{"term":{"allowReserved":true}}}}}],"responses":{"200":{"description":"stream","content":{"application/json-seq":{"$ref":"#/components/mediaTypes/EventStream"}}}}},"post":{"operationId":"upload","requestBody":{"content":{"multipart/mixed":{"schema":{"type":"array","prefixItems":[{"type":"string"},{"type":"array"}]},"prefixEncoding":[{"contentType":"text/plain","headers":{"Content-ID":{"required":true,"schema":{"type":"string"}}}},{"contentType":"multipart/mixed","prefixEncoding":[{"contentType":"application/json"}],"itemEncoding":{"contentType":"image/png"}}],"itemEncoding":{"contentType":"application/octet-stream"}}}},"responses":{"204":{"description":"ok"}}}}},"components":{"mediaTypes":{"EventStream":{"itemSchema":{"type":"object","properties":{"event":{"type":"string"}}}}},"securitySchemes":{"device":{"type":"oauth2","oauth2MetadataUrl":"https://auth.example/metadata","flows":{"deviceAuthorization":{"deviceAuthorizationUrl":"https://auth.example/device","tokenUrl":"https://auth.example/token","scopes":{"read":"Read"}}}}},"schemas":{"Xml":{"type":"string","xml":{"nodeType":"text"}}}}}`
	if err := os.WriteFile(input, []byte(spec), 0644); err != nil {
		t.Fatal(err)
	}
	if _, _, _, err := runWithHash(input, out, nil); err != nil {
		t.Fatal(err)
	}
	files, _ := os.ReadDir(filepath.Join(out, "operations"))
	operations := map[string]OperationDoc{}
	for _, file := range files {
		bytes, _ := os.ReadFile(filepath.Join(out, "operations", file.Name()))
		var op OperationDoc
		if err := json.Unmarshal(bytes, &op); err != nil {
			t.Fatal(err)
		}
		operations[op.OperationID] = op
	}
	search := operations["search"]
	if len(search.Parameters) != 1 || len(search.Parameters[0].Content) != 1 || !search.Parameters[0].Content[0].Encoding["term"].AllowReserved {
		t.Fatalf("query content lost: %#v", search.Parameters)
	}
	if search.Responses[0].ItemSchemaDefinition == nil {
		t.Fatal("stream item schema lost")
	}
	upload := operations["upload"].RequestBody.MediaTypes[0]
	if len(upload.PrefixEncoding) != 2 || upload.ItemEncoding.ContentType != "application/octet-stream" || upload.PrefixEncoding[1].ItemEncoding.ContentType != "image/png" {
		t.Fatalf("positional nesting lost: %#v", upload)
	}
	if upload.PrefixEncoding[0].Headers["Content-ID"].SchemaDefinition == nil {
		t.Fatal("part header schema lost")
	}
	bytes, err := os.ReadFile(filepath.Join(out, "security-schemes.json"))
	if err != nil {
		files, _ := os.ReadDir(out)
		t.Fatalf("security file: %v %v", err, files)
	}
	var security SecuritySchemesDoc
	if err = json.Unmarshal(bytes, &security); err != nil {
		t.Fatal(err)
	}
	flow := security.Schemes[0].OAuthFlows[0]
	if flow.Type != "deviceAuthorization" || flow.DeviceAuthorizationURL != "https://auth.example/device" || flow.TokenURL != "https://auth.example/token" {
		t.Fatalf("device authorization lost: %#v", flow)
	}
	metadataBytes, _ := os.ReadFile(filepath.Join(out, "api-metadata.json"))
	var metadata APIMetadataDoc
	if err = json.Unmarshal(metadataBytes, &metadata); err != nil {
		t.Fatal(err)
	}
	if len(metadata.Tags) != 1 || metadata.Tags[0].Parent != "api" || metadata.Tags[0].Kind != "resource" || metadata.Tags[0].Summary != "Events" {
		t.Fatalf("tag metadata lost: %#v", metadata)
	}
	schemaBytes, _ := os.ReadFile(filepath.Join(out, "schemas.json"))
	var catalog ComponentSchemasDoc
	if err = json.Unmarshal(schemaBytes, &catalog); err != nil {
		t.Fatal(err)
	}
	xml := catalog.Schemas[0].Schema.(map[string]any)["xml"].(map[string]any)
	if xml["nodeType"] != "text" {
		t.Fatalf("XML node metadata lost: %#v", xml)
	}
}

func TestOpenAPI32RejectsQueryLocationConflicts(t *testing.T) {
	for _, parameters := range []string{
		`[{"name":"whole","in":"querystring","content":{"application/json":{"schema":{"type":"object"}}}},{"name":"named","in":"query","schema":{"type":"string"}}]`,
		`[{"name":"whole","in":"querystring","content":{"application/json":{},"text/plain":{}}}]`,
		`[{"name":"whole","in":"querystring","content":{"application/json":{}}},{"name":"other","in":"querystring","content":{"application/json":{}}}]`,
	} {
		root := t.TempDir()
		input := filepath.Join(root, "api.json")
		spec := `{"openapi":"3.2.1","info":{"title":"Invalid","version":"1"},"paths":{"/search":{"get":{"parameters":` + parameters + `,"responses":{"200":{"description":"ok"}}}}}}`
		os.WriteFile(input, []byte(spec), 0644)
		if _, _, _, err := runWithHash(input, filepath.Join(root, "out"), nil); err == nil {
			t.Fatalf("accepted invalid query parameters: %s", parameters)
		}
	}
}
