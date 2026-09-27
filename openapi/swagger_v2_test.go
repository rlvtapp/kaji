package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

const swaggerV2Spec = `swagger: '2.0'
info:
  title: Legacy Pets
  version: '1.2.3'
host: api.example.test
basePath: /v1
schemes: [https]
consumes: [application/json]
produces: [application/json]
securityDefinitions:
  token:
    type: apiKey
    name: X-Token
    in: header
  basicAuth:
    type: basic
  oauth:
    type: oauth2
    flow: application
    tokenUrl: https://auth.example.test/token
    scopes:
      pets: Read pets
definitions:
  Pet:
    type: object
    required: [name]
    properties:
      name:
        type: string
paths:
  /pets/{petId}:
    parameters:
      - name: petId
        in: path
        required: true
        type: string
    get:
      operationId: getPet
      parameters:
        - name: include
          in: query
          type: array
          items: { type: string }
          collectionFormat: multi
      responses:
        '200':
          description: Found
          schema:
            $ref: '#/definitions/Pet'
          examples:
            application/json: { name: Miso }
    post:
      operationId: replacePet
      parameters:
        - name: body
          in: body
          required: true
          schema:
            $ref: '#/definitions/Pet'
      responses:
        '204':
          description: Replaced
`

func TestRunWithHashSupportsSwaggerV2(t *testing.T) {
	dir := t.TempDir()
	specPath := filepath.Join(dir, "swagger.yaml")
	if err := os.WriteFile(specPath, []byte(swaggerV2Spec), 0o644); err != nil {
		t.Fatalf("write Swagger spec: %v", err)
	}
	outDir := filepath.Join(dir, "out")
	_, changed, count, err := runWithHash(specPath, outDir, nil)
	if err != nil {
		t.Fatalf("compile Swagger 2.0: %v", err)
	}
	if !changed || count != 2 {
		t.Fatalf("expected two generated Swagger operations, changed=%v count=%d", changed, count)
	}

	var schemas ComponentSchemasDoc
	readSwaggerArtifact(t, filepath.Join(outDir, "schemas.json"), &schemas)
	if len(schemas.Schemas) != 1 || schemas.Schemas[0].Name != "Pet" {
		t.Fatalf("definitions were not normalized: %#v", schemas.Schemas)
	}
	var security SecuritySchemesDoc
	readSwaggerArtifact(t, filepath.Join(outDir, "security-schemes.json"), &security)
	var basic *SecuritySchemeDoc
	for index := range security.Schemes {
		if security.Schemes[index].Name == "basicAuth" {
			basic = &security.Schemes[index]
			break
		}
	}
	if len(security.Schemes) != 3 || basic == nil || basic.Type != "http" || basic.HTTPScheme != "basic" {
		t.Fatalf("security definitions were not normalized: %#v", security.Schemes)
	}

	var index map[string]string
	readSwaggerArtifact(t, filepath.Join(outDir, "operations.json"), &index)
	var get OperationDoc
	readSwaggerArtifact(t, filepath.Join(outDir, "operations", index["GET /pets/{petId}"]), &get)
	if len(get.Parameters) != 2 || get.Parameters[1].Style != "form" || get.Parameters[1].Explode == nil || !*get.Parameters[1].Explode {
		t.Fatalf("Swagger parameters were not normalized: %#v", get.Parameters)
	}
	if len(get.Responses) != 1 || get.Responses[0].ContentType != "application/json" || get.Responses[0].ExampleJSON == "" {
		t.Fatalf("Swagger response was not normalized: %#v", get.Responses)
	}
	var post OperationDoc
	readSwaggerArtifact(t, filepath.Join(outDir, "operations", index["POST /pets/{petId}"]), &post)
	if post.RequestBody == nil || !post.RequestBody.Required || len(post.RequestBody.MediaTypes) != 1 {
		t.Fatalf("Swagger body parameter was not normalized: %#v", post.RequestBody)
	}
}

func readSwaggerArtifact(t *testing.T, path string, target any) {
	t.Helper()
	contents, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	if err := json.Unmarshal(contents, target); err != nil {
		t.Fatalf("decode %s: %v", path, err)
	}
}
