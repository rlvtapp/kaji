package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/pb33f/libopenapi"
	v3 "github.com/pb33f/libopenapi/datamodel/high/v3"
)

const sampleSpec = `openapi: 3.1.0
info:
  title: Sample
  version: 1.0.0
paths:
  /items/{id}:
    parameters:
      - name: tenant
        in: header
        schema:
          type: string
    get:
      parameters:
        - name: search
          in: query
          schema:
            type: string
        - name: filter
          in: query
          schema:
            type: object
            properties:
              active:
                type: boolean
              category:
                type: string
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  items:
                    type: array
                    items:
                      type: object
                      properties:
                        id:
                          type: string
                        tags:
                          type: array
                          items:
                            type: string
                  total:
                    type: number
`

func loadSampleSpec(t *testing.T, spec string) *v3.Document {
	t.Helper()

	doc, err := libopenapi.NewDocument([]byte(spec))
	if err != nil {
		t.Fatalf("new document: %v", err)
	}

	model, err := doc.BuildV3Model()
	if err != nil {
		t.Fatalf("build model: %v", err)
	}

	docModel := model.Model
	return &docModel
}

func getOperation(t *testing.T, spec *v3.Document, method string) (*v3.PathItem, *v3.Operation) {
	t.Helper()

	if spec.Paths == nil || spec.Paths.PathItems == nil {
		t.Fatalf("paths not present")
	}

	for pair := spec.Paths.PathItems.First(); pair != nil; pair = pair.Next() {
		item := pair.Value()
		ops := item.GetOperations()
		if ops == nil {
			continue
		}
		for opPair := ops.First(); opPair != nil; opPair = opPair.Next() {
			if strings.EqualFold(opPair.Key(), method) {
				return item, opPair.Value()
			}
		}
	}

	t.Fatalf("operation %s not found", method)
	return nil, nil
}

func TestConvertResponsesProvidesDefaultExample(t *testing.T) {
	spec := loadSampleSpec(t, sampleSpec)
	_, op := getOperation(t, spec, "get")

	responses, err := convertResponses(op.Responses)
	if err != nil {
		t.Fatalf("convert responses: %v", err)
	}

	if len(responses) != 1 {
		t.Fatalf("expected 1 response, got %d", len(responses))
	}

	resp := responses[0]
	if resp.ContentType != "application/json" {
		t.Fatalf("expected application/json content type, got %s", resp.ContentType)
	}

	if resp.ExampleJSON == "" {
		t.Fatalf("expected example json")
	}
	if resp.SchemaDefinition == nil {
		t.Fatalf("expected lossless response schema definition")
	}

	var exampleObject map[string]any
	if err := json.Unmarshal([]byte(resp.ExampleJSON), &exampleObject); err != nil {
		t.Fatalf("example json is invalid: %v", err)
	}

	total, ok := exampleObject["total"].(string)
	if !ok {
		t.Fatalf("total placeholder missing: %#v", exampleObject["total"])
	}
	if total != "<number>" {
		t.Fatalf("unexpected total placeholder: %s", total)
	}

	itemsValue, ok := exampleObject["items"].([]any)
	if !ok {
		t.Fatalf("items placeholder missing")
	}
	if len(itemsValue) == 0 {
		t.Fatalf("items placeholder empty")
	}

	firstItem, ok := itemsValue[0].(map[string]any)
	if !ok {
		t.Fatalf("first item is not an object: %#v", itemsValue[0])
	}

	idVal, ok := firstItem["id"].(string)
	if !ok || idVal != "<string>" {
		t.Fatalf("unexpected id placeholder: %#v", firstItem["id"])
	}

	tagsVal, ok := firstItem["tags"].([]any)
	if !ok || len(tagsVal) == 0 {
		t.Fatalf("tags placeholder missing: %#v", firstItem["tags"])
	}
	if tag, ok := tagsVal[0].(string); !ok || tag != "<string>" {
		t.Fatalf("unexpected tag placeholder: %#v", tagsVal[0])
	}
}

func TestOperationDocsPreserveRequestAndResponseSchemaDefinitions(t *testing.T) {
	spec := loadSampleSpec(t, sampleSpec)
	_, op := getOperation(t, spec, "get")

	responses, err := convertResponses(op.Responses)
	if err != nil {
		t.Fatalf("convert responses: %v", err)
	}
	if len(responses) != 1 || responses[0].SchemaDefinition == nil {
		t.Fatalf("response schema definition missing: %#v", responses)
	}

	request := &v3.RequestBody{}
	request.Content = op.Responses.Codes.GetOrZero("200").Content
	body, err := convertRequestBody(request)
	if err != nil {
		t.Fatalf("convert request body: %v", err)
	}
	if body == nil || body.SchemaDefinition == nil {
		t.Fatalf("request schema definition missing: %#v", body)
	}
}

func TestRunPreservesCompleteOperationSdkContract(t *testing.T) {
	const spec = `openapi: 3.1.0
info:
  title: SDK contract
  version: 1.0.0
components:
  securitySchemes:
    oauth:
      type: oauth2
      flows:
        clientCredentials:
          tokenUrl: https://example.test/token
          scopes:
            pets:write: Write pets
    api_key:
      type: apiKey
      name: X-API-Key
      in: header
paths:
  /pets/{petId}:
    post:
      parameters:
        - name: petId
          in: path
          required: true
          style: matrix
          explode: true
          schema:
            type: string
        - name: include
          in: query
          style: pipeDelimited
          explode: false
          schema:
            type: array
            items:
              type: string
      security:
        - oauth: [pets:write]
          api_key: []
        - {}
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                name:
                  type: string
          application/xml:
            schema:
              type: string
      responses:
        '201':
          description: Created
          content:
            application/json:
              schema:
                type: object
            application/xml:
              schema:
                type: string
        default:
          description: Failure
`
	dir := t.TempDir()
	specPath := filepath.Join(dir, "spec.yaml")
	outDir := filepath.Join(dir, "out")
	if err := os.WriteFile(specPath, []byte(spec), 0o644); err != nil {
		t.Fatalf("write spec: %v", err)
	}
	if _, _, _, err := runWithHash(specPath, outDir, nil); err != nil {
		t.Fatalf("run: %v", err)
	}

	indexBytes, err := os.ReadFile(filepath.Join(outDir, "operations.json"))
	if err != nil {
		t.Fatalf("read index: %v", err)
	}
	var index map[string]string
	if err := json.Unmarshal(indexBytes, &index); err != nil {
		t.Fatalf("decode index: %v", err)
	}
	documentBytes, err := os.ReadFile(filepath.Join(outDir, "operations", index["POST /pets/{petId}"]))
	if err != nil {
		t.Fatalf("read operation: %v", err)
	}
	var document OperationDoc
	if err := json.Unmarshal(documentBytes, &document); err != nil {
		t.Fatalf("decode operation: %v", err)
	}
	if len(document.Parameters) != 2 || document.Parameters[0].In != "path" || !document.Parameters[0].Required {
		t.Fatalf("parameters were not preserved: %#v", document.Parameters)
	}
	if document.Parameters[0].Style != "matrix" || document.Parameters[0].Explode == nil || !*document.Parameters[0].Explode {
		t.Fatalf("path parameter style/explode were not preserved: %#v", document.Parameters[0])
	}
	if document.Parameters[1].Style != "pipeDelimited" || document.Parameters[1].Explode == nil || *document.Parameters[1].Explode {
		t.Fatalf("query parameter style/explode were not preserved: %#v", document.Parameters[1])
	}
	if document.RequestBody == nil || !document.RequestBody.Required || len(document.RequestBody.MediaTypes) != 2 {
		t.Fatalf("request media types were not preserved: %#v", document.RequestBody)
	}
	if len(document.Responses) != 3 || document.Responses[0].Code != "201" || document.Responses[1].ContentType != "application/xml" {
		t.Fatalf("response status/media types were not preserved: %#v", document.Responses)
	}
	if len(document.SecurityRequirements) != 2 {
		t.Fatalf("security alternatives were not preserved: %#v", document.SecurityRequirements)
	}
	if got := document.SecurityRequirements[0].Schemes["oauth"]; len(got) != 1 || got[0] != "pets:write" {
		t.Fatalf("oauth scopes were not preserved: %#v", document.SecurityRequirements)
	}
	if len(document.SecurityRequirements[1].Schemes) != 0 {
		t.Fatalf("anonymous security alternative was not preserved: %#v", document.SecurityRequirements[1])
	}
}

func TestRunEmitsSecuritySchemeCatalogForSDKConsumers(t *testing.T) {
	const spec = `openapi: 3.1.0
info:
  title: Security catalog
  version: 1.0.0
components:
  securitySchemes:
    ApiKey:
      type: apiKey
      name: X-API-Key
      in: header
      description: Tenant key
    Bearer:
      type: http
      scheme: bearer
      bearerFormat: JWT
    OAuth:
      type: oauth2
      oauth2MetadataUrl: https://example.test/metadata
      flows:
        authorizationCode:
          authorizationUrl: https://example.test/authorize
          tokenUrl: https://example.test/token
          refreshUrl: https://example.test/refresh
          scopes:
            pets:read: Read pets
paths: {}
`
	dir := t.TempDir()
	specPath := filepath.Join(dir, "spec.yaml")
	outDir := filepath.Join(dir, "out")
	if err := os.WriteFile(specPath, []byte(spec), 0o644); err != nil {
		t.Fatalf("write spec: %v", err)
	}
	if _, _, _, err := runWithHash(specPath, outDir, nil); err != nil {
		t.Fatalf("run: %v", err)
	}

	bytes, err := os.ReadFile(filepath.Join(outDir, "security-schemes.json"))
	if err != nil {
		t.Fatalf("read security scheme catalog: %v", err)
	}
	var catalog SecuritySchemesDoc
	if err := json.Unmarshal(bytes, &catalog); err != nil {
		t.Fatalf("decode security scheme catalog: %v", err)
	}
	if len(catalog.Schemes) != 3 {
		t.Fatalf("expected three security schemes, got %#v", catalog.Schemes)
	}
	if scheme := catalog.Schemes[0]; scheme.Name != "ApiKey" || scheme.Type != "apiKey" || scheme.APIKeyName != "X-API-Key" || scheme.APIKeyIn != "header" {
		t.Fatalf("api key metadata was not preserved: %#v", scheme)
	}
	if scheme := catalog.Schemes[1]; scheme.Name != "Bearer" || scheme.HTTPScheme != "bearer" || scheme.BearerFormat != "JWT" {
		t.Fatalf("HTTP metadata was not preserved: %#v", scheme)
	}
	if scheme := catalog.Schemes[2]; scheme.Name != "OAuth" || scheme.OAuth2MetadataURL != "https://example.test/metadata" || len(scheme.OAuthFlows) != 1 || scheme.OAuthFlows[0].Type != "authorizationCode" || scheme.OAuthFlows[0].Scopes["pets:read"] != "Read pets" {
		t.Fatalf("OAuth metadata was not preserved: %#v", scheme)
	}
}

func TestGenerateExampleFromSchemaProducesPlaceholders(t *testing.T) {
	spec := loadSampleSpec(t, sampleSpec)
	_, op := getOperation(t, spec, "get")

	var filterParam *v3.Parameter
	for _, param := range op.Parameters {
		if param == nil {
			continue
		}
		if param.Name == "filter" {
			filterParam = param
			break
		}
	}

	if filterParam == nil || filterParam.Schema == nil {
		t.Fatalf("filter parameter schema missing")
	}

	example, err := generateExampleFromSchemaProxy(filterParam.Schema)
	if err != nil {
		t.Fatalf("generate example: %v", err)
	}

	exampleMap, ok := example.(map[string]any)
	if !ok {
		t.Fatalf("filter example is not an object: %#v", example)
	}

	if val, ok := exampleMap["active"].(string); !ok || val != "<boolean>" {
		t.Fatalf("unexpected active placeholder: %#v", exampleMap["active"])
	}

	if val, ok := exampleMap["category"].(string); !ok || val != "<string>" {
		t.Fatalf("unexpected category placeholder: %#v", exampleMap["category"])
	}
}

func TestRunWithHashSkipsWhenUnchanged(t *testing.T) {
	dir := t.TempDir()
	specPath := filepath.Join(dir, "spec.yaml")
	outDir := filepath.Join(dir, "out")

	if err := os.WriteFile(specPath, []byte(sampleSpec), 0o644); err != nil {
		t.Fatalf("write spec: %v", err)
	}

	hash1, changed1, count1, err := runWithHash(specPath, outDir, nil)
	if err != nil {
		t.Fatalf("first run: %v", err)
	}
	if !changed1 {
		t.Fatalf("expected first run to report changes")
	}
	if count1 == 0 {
		t.Fatalf("expected operations to be generated")
	}

	hash2, changed2, _, err := runWithHash(specPath, outDir, &hash1)
	if err != nil {
		t.Fatalf("second run: %v", err)
	}
	if changed2 {
		t.Fatalf("expected second run to detect no changes")
	}
	if hash2 != hash1 {
		t.Fatalf("hash mismatch: %016x vs %016x", hash2, hash1)
	}

	if _, err := os.Stat(filepath.Join(outDir, "operations.json")); err != nil {
		t.Fatalf("operations index missing after cached run: %v", err)
	}

	modified := strings.ReplaceAll(sampleSpec, "Sample", "Sample Updated")
	if err := os.WriteFile(specPath, []byte(modified), 0o644); err != nil {
		t.Fatalf("update spec: %v", err)
	}

	hash3, changed3, _, err := runWithHash(specPath, outDir, &hash2)
	if err != nil {
		t.Fatalf("third run: %v", err)
	}
	if !changed3 {
		t.Fatalf("expected third run to detect changes")
	}
	if hash3 == hash2 {
		t.Fatalf("hash should change after spec update")
	}
}

func TestRunWithHashGeneratesWebhookEntries(t *testing.T) {
	dir := t.TempDir()
	specPath := filepath.Join(dir, "spec.yaml")
	outDir := filepath.Join(dir, "out")
	spec := `openapi: 3.1.0
info:
  title: Webhooks
  version: 1.0.0
webhooks:
  orderUpdated:
    post:
      summary: Order updated
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                orderId:
                  type: string
      responses:
        '200':
          description: OK
`

	if err := os.WriteFile(specPath, []byte(spec), 0o644); err != nil {
		t.Fatalf("write spec: %v", err)
	}

	_, changed, count, err := runWithHash(specPath, outDir, nil)
	if err != nil {
		t.Fatalf("runWithHash: %v", err)
	}
	if !changed {
		t.Fatalf("expected webhook spec to report changes")
	}
	if count != 1 {
		t.Fatalf("expected 1 generated webhook operation, got %d", count)
	}

	indexBytes, err := os.ReadFile(filepath.Join(outDir, "operations.json"))
	if err != nil {
		t.Fatalf("read operations index: %v", err)
	}

	var index map[string]string
	if err := json.Unmarshal(indexBytes, &index); err != nil {
		t.Fatalf("decode operations index: %v", err)
	}

	webhookFile, ok := index["WEBHOOK orderUpdated"]
	if !ok {
		t.Fatalf("missing simple webhook alias in index: %#v", index)
	}
	if _, ok := index["WEBHOOK POST orderUpdated"]; !ok {
		t.Fatalf("missing method-specific webhook alias in index: %#v", index)
	}

	docBytes, err := os.ReadFile(filepath.Join(outDir, "operations", webhookFile))
	if err != nil {
		t.Fatalf("read webhook operation doc: %v", err)
	}

	var doc OperationDoc
	if err := json.Unmarshal(docBytes, &doc); err != nil {
		t.Fatalf("decode webhook operation doc: %v", err)
	}

	if doc.Kind != "webhook" {
		t.Fatalf("expected webhook kind, got %q", doc.Kind)
	}
	if doc.Name != "orderUpdated" {
		t.Fatalf("expected webhook name, got %q", doc.Name)
	}
	if doc.Method != "POST" {
		t.Fatalf("expected POST webhook method, got %q", doc.Method)
	}
}

func TestRunWithHashWritesLosslessComponentSchemas(t *testing.T) {
	dir := t.TempDir()
	specPath := filepath.Join(dir, "spec.yaml")
	outDir := filepath.Join(dir, "out")
	spec := `openapi: 3.1.0
info:
  title: Components
  version: 1.0.0
paths: {}
components:
  schemas:
    Pet:
      type: object
      required: [name]
      properties:
        name:
          type: string
        age:
          type: integer
    PetList:
      type: array
      items:
        $ref: '#/components/schemas/Pet'
`
	if err := os.WriteFile(specPath, []byte(spec), 0o644); err != nil {
		t.Fatalf("write spec: %v", err)
	}

	if _, _, _, err := runWithHash(specPath, outDir, nil); err != nil {
		t.Fatalf("runWithHash: %v", err)
	}

	bytes, err := os.ReadFile(filepath.Join(outDir, "schemas.json"))
	if err != nil {
		t.Fatalf("read schemas: %v", err)
	}
	var document ComponentSchemasDoc
	if err := json.Unmarshal(bytes, &document); err != nil {
		t.Fatalf("decode schemas: %v", err)
	}
	if len(document.Schemas) != 2 {
		t.Fatalf("expected two component schemas, got %d", len(document.Schemas))
	}
	pet, ok := document.Schemas[0].Schema.(map[string]any)
	if !ok {
		t.Fatalf("pet schema is not an object: %#v", document.Schemas[0].Schema)
	}
	if pet["type"] != "object" {
		t.Fatalf("expected object component schema, got %#v", pet["type"])
	}
	properties, ok := pet["properties"].(map[string]any)
	if !ok || properties["name"] == nil {
		t.Fatalf("component properties were not preserved: %#v", pet["properties"])
	}
}
