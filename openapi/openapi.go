package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"unicode"

	"github.com/cespare/xxhash/v2"
	"github.com/pb33f/libopenapi"
	"github.com/pb33f/libopenapi/datamodel/high/base"

	v3 "github.com/pb33f/libopenapi/datamodel/high/v3"
	"github.com/pb33f/libopenapi/orderedmap"

	yaml "go.yaml.in/yaml/v4"
)

func run(specPath, outDir string) error {
	hash, _, count, err := runWithHash(specPath, outDir, nil)
	if err != nil {
		return err
	}

	fmt.Printf("Processed %d operations into %s (hash %016x)\n", count, outDir, hash)
	return nil
}

func runWithHash(specPath, outDir string, prevHash *uint64) (uint64, bool, int, error) {
	data, err := os.ReadFile(specPath)
	if err != nil {
		return 0, false, 0, fmt.Errorf("read spec: %w", err)
	}

	newHash := computeSpecHash(data)

	if prevHash != nil && *prevHash == newHash {
		return newHash, false, 0, nil
	}

	doc, err := libopenapi.NewDocument(data)
	if err != nil {
		return 0, false, 0, fmt.Errorf("parse spec: %w", err)
	}

	model, err := doc.BuildV3Model()
	if err != nil {
		return 0, false, 0, fmt.Errorf("build v3 model: %w", err)
	}

	spec := model.Model

	if err := os.MkdirAll(outDir, 0o755); err != nil {
		return 0, false, 0, fmt.Errorf("create output directory: %w", err)
	}

	operationsDir := filepath.Join(outDir, "operations")
	if err := os.RemoveAll(operationsDir); err != nil && !os.IsNotExist(err) {
		return 0, false, 0, fmt.Errorf("cleanup operations dir: %w", err)
	}
	if err := os.MkdirAll(operationsDir, 0o755); err != nil {
		return 0, false, 0, fmt.Errorf("create operations dir: %w", err)
	}

	componentSchemas, err := collectComponentSchemas(spec.Components)
	if err != nil {
		return 0, false, 0, fmt.Errorf("convert component schemas: %w", err)
	}
	if err := writeJSON(filepath.Join(outDir, "schemas.json"), ComponentSchemasDoc{Schemas: componentSchemas}); err != nil {
		return 0, false, 0, fmt.Errorf("write component schemas: %w", err)
	}
	securitySchemes := collectSecuritySchemes(spec.Components)
	if err := writeJSON(filepath.Join(outDir, "security-schemes.json"), SecuritySchemesDoc{Schemes: securitySchemes}); err != nil {
		return 0, false, 0, fmt.Errorf("write security schemes: %w", err)
	}

	index := make(map[string]string)
	order := make([]string, 0)

	var defaultSecurity []*base.SecurityRequirement
	if spec.Security != nil {
		defaultSecurity = spec.Security
	}

	count := 0

	if spec.Paths != nil && spec.Paths.PathItems != nil {
		generated, err := collectPathItemOperations(spec.Paths.PathItems, defaultSecurity, spec.Components, spec.Servers, outDir, index, &order, false)
		if err != nil {
			return 0, false, 0, err
		}
		count += generated
	}

	if spec.Webhooks != nil {
		generated, err := collectPathItemOperations(spec.Webhooks, defaultSecurity, spec.Components, spec.Servers, outDir, index, &order, true)
		if err != nil {
			return 0, false, 0, err
		}
		count += generated
	}

	if err := writeJSON(filepath.Join(outDir, "operations.json"), index); err != nil {
		return 0, false, 0, fmt.Errorf("write operation index: %w", err)
	}
	if err := writeJSON(filepath.Join(outDir, "operations-order.json"), order); err != nil {
		return 0, false, 0, fmt.Errorf("write operation order: %w", err)
	}

	return newHash, true, count, nil
}

func computeSpecHash(data []byte) uint64 {
	return xxhash.Sum64(data)
}

func collectComponentSchemas(components *v3.Components) ([]ComponentSchemaDoc, error) {
	if components == nil || components.Schemas == nil {
		return []ComponentSchemaDoc{}, nil
	}

	schemas := make([]ComponentSchemaDoc, 0, orderedmap.Len(components.Schemas))
	for pair := components.Schemas.First(); pair != nil; pair = pair.Next() {
		schema, err := schemaProxyToInterface(pair.Value())
		if err != nil {
			return nil, fmt.Errorf("render component schema %q: %w", pair.Key(), err)
		}
		if schema == nil {
			return nil, fmt.Errorf("component schema %q resolved to nil", pair.Key())
		}
		schemas = append(schemas, ComponentSchemaDoc{
			Name:   pair.Key(),
			Schema: schema,
		})
	}

	return schemas, nil
}

// collectSecuritySchemes writes the reusable component catalog separately from
// operation requirements. Requirements carry only names/scopes, while SDK
// clients need this catalog to know whether a credential belongs in a header,
// query, cookie, or an HTTP/OAuth authorization flow.
func collectSecuritySchemes(components *v3.Components) []SecuritySchemeDoc {
	if components == nil || components.SecuritySchemes == nil {
		return []SecuritySchemeDoc{}
	}

	schemes := make([]SecuritySchemeDoc, 0, orderedmap.Len(components.SecuritySchemes))
	for pair := components.SecuritySchemes.First(); pair != nil; pair = pair.Next() {
		scheme := pair.Value()
		if scheme == nil {
			continue
		}
		schemes = append(schemes, SecuritySchemeDoc{
			Name:              pair.Key(),
			Type:              scheme.Type,
			Description:       scheme.Description,
			APIKeyName:        scheme.Name,
			APIKeyIn:          scheme.In,
			HTTPScheme:        scheme.Scheme,
			BearerFormat:      scheme.BearerFormat,
			OAuthFlows:        collectOAuthFlows(scheme.Flows),
			OpenIDConnectURL:  scheme.OpenIdConnectUrl,
			OAuth2MetadataURL: scheme.OAuth2MetadataUrl,
		})
	}
	return schemes
}

func collectOAuthFlows(flows *v3.OAuthFlows) []OAuthFlowDoc {
	if flows == nil {
		return nil
	}

	result := make([]OAuthFlowDoc, 0, 5)
	appendFlow := func(kind string, flow *v3.OAuthFlow) {
		if flow == nil {
			return
		}
		var scopes map[string]string
		if flow.Scopes != nil {
			scopes = make(map[string]string, orderedmap.Len(flow.Scopes))
			for pair := flow.Scopes.First(); pair != nil; pair = pair.Next() {
				scopes[pair.Key()] = pair.Value()
			}
		}
		result = append(result, OAuthFlowDoc{
			Type:             kind,
			AuthorizationURL: flow.AuthorizationUrl,
			TokenURL:         flow.TokenUrl,
			RefreshURL:       flow.RefreshUrl,
			Scopes:           scopes,
		})
	}

	// OpenAPI defines these flow names. Fixed ordering makes the emitted JSON
	// deterministic regardless of how the source document was written.
	appendFlow("implicit", flows.Implicit)
	appendFlow("password", flows.Password)
	appendFlow("clientCredentials", flows.ClientCredentials)
	appendFlow("authorizationCode", flows.AuthorizationCode)
	appendFlow("device", flows.Device)
	return result
}

func collectPathItemOperations(
	items *orderedmap.Map[string, *v3.PathItem],
	defaultSecurity []*base.SecurityRequirement,
	components *v3.Components,
	specServers []*v3.Server,
	outDir string,
	index map[string]string,
	order *[]string,
	isWebhook bool,
) (int, error) {
	count := 0

	for pair := items.First(); pair != nil; pair = pair.Next() {
		path := pair.Key()
		item := pair.Value()
		if item == nil {
			continue
		}

		pathParams, err := convertParameterList(item.Parameters)
		if err != nil {
			if isWebhook {
				return 0, fmt.Errorf("convert webhook parameters (%s): %w", path, err)
			}
			return 0, fmt.Errorf("convert path parameters (%s): %w", path, err)
		}

		operations := item.GetOperations()
		if operations == nil {
			continue
		}

		methods := make([]string, 0)
		for opPair := operations.First(); opPair != nil; opPair = opPair.Next() {
			method := strings.ToLower(opPair.Key())
			op := opPair.Value()
			if op == nil {
				continue
			}
			methods = append(methods, method)

			opParams, err := convertParameterList(op.Parameters)
			if err != nil {
				if isWebhook {
					return 0, fmt.Errorf("convert webhook parameters (%s %s): %w", method, path, err)
				}
				return 0, fmt.Errorf("convert operation parameters (%s %s): %w", method, path, err)
			}

			responses, err := convertResponses(op.Responses)
			if err != nil {
				if isWebhook {
					return 0, fmt.Errorf("convert webhook responses (%s %s): %w", method, path, err)
				}
				return 0, fmt.Errorf("convert responses (%s %s): %w", method, path, err)
			}

			requestExamples, err := convertRequestExamples(op.RequestBody)
			if err != nil {
				if isWebhook {
					return 0, fmt.Errorf("convert webhook request examples (%s %s): %w", method, path, err)
				}
				return 0, fmt.Errorf("convert request examples (%s %s): %w", method, path, err)
			}

			requestBody, err := convertRequestBody(op.RequestBody)
			if err != nil {
				if isWebhook {
					return 0, fmt.Errorf("convert webhook request body (%s %s): %w", method, path, err)
				}
				return 0, fmt.Errorf("convert request body (%s %s): %w", method, path, err)
			}

			parameters := mergeParameters(pathParams, opParams)

			auth := buildAuthDoc(op.Security, defaultSecurity, components)
			securityRequirements := convertSecurityRequirements(op.Security, defaultSecurity)
			servers := resolveServers(op.Servers, item.Servers, specServers)
			serverDocs := convertServers(servers)
			deprecated := false
			if op.Deprecated != nil {
				deprecated = *op.Deprecated
			}

			opDoc := OperationDoc{
				OperationID:          op.OperationId,
				Path:                 path,
				Method:               strings.ToUpper(method),
				Summary:              op.Summary,
				Description:          op.Description,
				Deprecated:           deprecated,
				Hidden:               extensionBool(op.Extensions, "x-hidden"),
				Auth:                 auth,
				SecurityRequirements: securityRequirements,
				Servers:              serverDocs,
				Parameters:           parameters,
				RequestBody:          requestBody,
				Responses:            responses,
				RequestExamples:      requestExamples,
				Extensions:           convertExtensions(op.Extensions),
			}

			if isWebhook {
				opDoc.Kind = "webhook"
				opDoc.Name = path
			} else {
				opDoc.Kind = "operation"
				opDoc.Name = strings.ToUpper(method) + " " + path
			}

			slug := slugFor(path, method)
			if isWebhook {
				slug = "webhook_" + slug
			}
			relFile := filepath.Join("operations", slug+".json")
			absFile := filepath.Join(outDir, relFile)

			if err := writeJSON(absFile, opDoc); err != nil {
				if isWebhook {
					return 0, fmt.Errorf("write webhook file (%s %s): %w", method, path, err)
				}
				return 0, fmt.Errorf("write operation file (%s %s): %w", method, path, err)
			}

			if isWebhook {
				key := "WEBHOOK " + strings.ToUpper(method) + " " + path
				index[key] = slug + ".json"
				*order = append(*order, key)
			} else {
				key := strings.ToUpper(method) + " " + path
				index[key] = slug + ".json"
				*order = append(*order, key)
			}
			count++
		}

		if isWebhook && len(methods) == 1 {
			sort.Strings(methods)
			slug := "webhook_" + slugFor(path, methods[0])
			index["WEBHOOK "+path] = slug + ".json"
		}
	}

	return count, nil
}

func convertExtensions(extensions *orderedmap.Map[string, *yaml.Node]) map[string]any {
	if orderedmap.Len(extensions) == 0 {
		return nil
	}

	out := make(map[string]any)
	for pair := extensions.First(); pair != nil; pair = pair.Next() {
		key := pair.Key()
		// The docs renderer owns x-mint/x-rlvt, while Kaji consumes its own
		// operation-level contract extensions after this sidecar has normalized
		// the OpenAPI document. Keep that boundary explicit rather than passing
		// every arbitrary vendor extension to downstream generators.
		if key != "x-mint" && key != "x-rlvt" && !strings.HasPrefix(key, "x-kaji-") {
			continue
		}
		value, err := yamlNodeToInterface(pair.Value())
		if err != nil || value == nil {
			continue
		}
		out[key] = value
	}

	if len(out) == 0 {
		return nil
	}

	return out
}

func extensionBool(extensions *orderedmap.Map[string, *yaml.Node], key string) bool {
	if orderedmap.Len(extensions) == 0 {
		return false
	}
	node := extensions.GetOrZero(key)
	if node == nil {
		return false
	}
	value, err := yamlNodeToInterface(node)
	if err != nil {
		return false
	}
	boolValue, ok := value.(bool)
	return ok && boolValue
}

func convertParameterList(params []*v3.Parameter) ([]ParameterDoc, error) {
	if len(params) == 0 {
		return nil, nil
	}

	out := make([]ParameterDoc, 0, len(params))
	for _, param := range params {
		if param == nil {
			continue
		}
		doc, err := convertParameter(param)
		if err != nil {
			return nil, err
		}
		out = append(out, doc)
	}

	if len(out) == 0 {
		return nil, nil
	}

	return out, nil
}

func convertParameter(param *v3.Parameter) (ParameterDoc, error) {
	doc := ParameterDoc{
		Name:        param.Name,
		In:          param.In,
		Description: param.Description,
		Style:       param.Style,
		Explode:     param.Explode,
		Required:    false,
	}

	if param.Required != nil {
		doc.Required = *param.Required
	} else if strings.EqualFold(param.In, "path") {
		doc.Required = true
	}

	if param.Schema != nil {
		schema, err := schemaProxyToInterface(param.Schema)
		if err != nil {
			return doc, err
		}
		if schema != nil {
			doc.Schema = schema
		}
	}

	return doc, nil
}
func convertResponses(responses *v3.Responses) ([]ResponseDoc, error) {
	if responses == nil {
		return nil, nil
	}

	var docs []ResponseDoc

	if orderedmap.Len(responses.Codes) > 0 {
		for pair := responses.Codes.First(); pair != nil; pair = pair.Next() {
			code := pair.Key()
			response := pair.Value()
			entries, err := convertResponse(code, response)
			if err != nil {
				return nil, err
			}
			docs = append(docs, entries...)
		}
	}

	if responses.Default != nil {
		entries, err := convertResponse("default", responses.Default)
		if err != nil {
			return nil, err
		}
		docs = append(docs, entries...)
	}

	if len(docs) == 0 {
		return nil, nil
	}

	return docs, nil
}

func convertRequestExamples(requestBody *v3.RequestBody) ([]ExampleDoc, error) {
	if requestBody == nil || requestBody.Content == nil {
		return nil, nil
	}

	if orderedmap.Len(requestBody.Content) == 0 {
		return nil, nil
	}

	var docs []ExampleDoc
	for pair := requestBody.Content.First(); pair != nil; pair = pair.Next() {
		contentType := pair.Key()
		mediaType := pair.Value()
		if mediaType == nil {
			continue
		}

		// A Media Type Object can carry several explicitly named examples. The
		// docs playground exposes these as selectable request bodies, so do not
		// reduce an OpenAPI examples map to its first entry here. The single
		// `example` field and schema-derived fallback remain one request example.
		if orderedmap.Len(mediaType.Examples) > 0 {
			for examplePair := mediaType.Examples.First(); examplePair != nil; examplePair = examplePair.Next() {
				exampleValue, err := extractExampleValue(examplePair.Value())
				if err != nil {
					return nil, err
				}
				exampleJSON := formatExampleJSON(exampleValue)
				if exampleJSON == "" {
					continue
				}
				docs = append(docs, ExampleDoc{
					Label:       requestExampleLabel(contentType, examplePair.Key()),
					ContentType: contentType,
					ExampleJSON: exampleJSON,
				})
			}
			continue
		}

		exampleValue, err := buildExampleForMediaType(mediaType)
		if err != nil {
			return nil, err
		}
		exampleJSON := formatExampleJSON(exampleValue)
		if exampleJSON == "" {
			continue
		}
		docs = append(docs, ExampleDoc{
			Label:       requestExampleLabel(contentType, ""),
			ContentType: contentType,
			ExampleJSON: exampleJSON,
		})
	}

	if len(docs) == 0 {
		return nil, nil
	}

	return docs, nil
}

func requestExampleLabel(contentType, name string) string {
	label := "Request"
	if contentType != "" {
		label += " " + contentType
	}
	if name != "" {
		label += ": " + name
	}
	return label
}

func convertRequestBody(requestBody *v3.RequestBody) (*BodyDoc, error) {
	if requestBody == nil || requestBody.Content == nil || orderedmap.Len(requestBody.Content) == 0 {
		return nil, nil
	}

	var mediaTypes []MediaTypeDoc
	var first *BodyDoc
	for pair := requestBody.Content.First(); pair != nil; pair = pair.Next() {
		contentType := pair.Key()
		mediaType := pair.Value()
		if mediaType == nil {
			continue
		}

		schema, err := schemaFieldsFromMediaType(mediaType)
		if err != nil {
			return nil, err
		}
		schemaDefinition, err := schemaDefinitionFromMediaType(mediaType)
		if err != nil {
			return nil, err
		}
		exampleValue, err := buildExampleForMediaType(mediaType)
		if err != nil {
			return nil, err
		}
		mediaTypeDoc := MediaTypeDoc{
			ContentType:      contentType,
			Schema:           schema,
			SchemaDefinition: schemaDefinition,
			ExampleJSON:      formatExampleJSON(exampleValue),
		}
		mediaTypes = append(mediaTypes, mediaTypeDoc)

		if first == nil {
			first = &BodyDoc{
				Description:      requestBody.Description,
				Required:         requestBody.Required != nil && *requestBody.Required,
				ContentType:      mediaTypeDoc.ContentType,
				Schema:           mediaTypeDoc.Schema,
				SchemaDefinition: mediaTypeDoc.SchemaDefinition,
				ExampleJSON:      mediaTypeDoc.ExampleJSON,
			}
		}
	}

	if first == nil {
		return nil, nil
	}
	first.MediaTypes = mediaTypes
	return first, nil
}

func convertResponse(code string, response *v3.Response) ([]ResponseDoc, error) {
	if response == nil {
		return nil, nil
	}

	var docs []ResponseDoc

	if response.Content != nil && orderedmap.Len(response.Content) > 0 {
		for pair := response.Content.First(); pair != nil; pair = pair.Next() {
			contentType := pair.Key()
			mediaType := pair.Value()

			schemaFields, err := schemaFieldsFromMediaType(mediaType)
			if err != nil {
				return nil, err
			}
			schemaDefinition, err := schemaDefinitionFromMediaType(mediaType)
			if err != nil {
				return nil, err
			}
			exampleValue, err := buildExampleForMediaType(mediaType)
			if err != nil {
				return nil, err
			}

			exampleJSON := formatExampleJSON(exampleValue)

			docs = append(docs, ResponseDoc{
				Code:             code,
				Description:      response.Description,
				ContentType:      contentType,
				Schema:           schemaFields,
				SchemaDefinition: schemaDefinition,
				ExampleJSON:      exampleJSON,
			})
		}
	}

	if len(docs) == 0 {
		docs = append(docs, ResponseDoc{
			Code:        code,
			Description: response.Description,
		})
	}

	return docs, nil
}

func schemaDefinitionFromMediaType(mediaType *v3.MediaType) (any, error) {
	if mediaType == nil || mediaType.Schema == nil {
		return nil, nil
	}
	return schemaProxyToInterface(mediaType.Schema)
}

func schemaFieldsFromMediaType(mediaType *v3.MediaType) ([]SchemaField, error) {
	if mediaType == nil {
		return nil, nil
	}

	if mediaType.Schema == nil {
		return nil, nil
	}

	schema := mediaType.Schema.Schema()
	if schema == nil {
		if err := mediaType.Schema.GetBuildError(); err != nil {
			return nil, err
		}
		return nil, nil
	}

	return schemaToFields(schema), nil
}

func buildExampleForMediaType(mediaType *v3.MediaType) (any, error) {
	if mediaType == nil {
		return nil, nil
	}

	if mediaType.Example != nil {
		value, err := yamlNodeToInterface(mediaType.Example)
		if err != nil {
			return nil, err
		}
		if value != nil {
			return value, nil
		}
	}

	if orderedmap.Len(mediaType.Examples) > 0 {
		var fallback any
		for pair := mediaType.Examples.First(); pair != nil; pair = pair.Next() {
			value, err := extractExampleValue(pair.Value())
			if err != nil {
				return nil, err
			}
			if value == nil {
				continue
			}
			if pair.Key() == "default" {
				return value, nil
			}
			if fallback == nil {
				fallback = value
			}
		}
		if fallback != nil {
			return fallback, nil
		}
	}

	if mediaType.Schema != nil {
		return generateExampleFromSchemaProxy(mediaType.Schema)
	}

	return nil, nil
}

func formatExampleJSON(value any) string {
	if value == nil {
		return ""
	}

	switch v := value.(type) {
	case string:
		return fmt.Sprintf("%q", v)
	default:
		buf := &bytes.Buffer{}
		encoder := json.NewEncoder(buf)
		encoder.SetEscapeHTML(false)
		encoder.SetIndent("", "  ")
		if err := encoder.Encode(value); err != nil {
			return fmt.Sprintf("%v", value)
		}
		result := strings.TrimSuffix(buf.String(), "\n")
		return result
	}
}

func resolveServers(operationServers, pathServers, rootServers []*v3.Server) []*v3.Server {
	if len(operationServers) > 0 {
		return operationServers
	}
	if len(pathServers) > 0 {
		return pathServers
	}
	if len(rootServers) > 0 {
		return rootServers
	}
	return nil
}

func convertServers(servers []*v3.Server) []ServerDoc {
	if len(servers) == 0 {
		return nil
	}

	docs := make([]ServerDoc, 0, len(servers))
	for _, server := range servers {
		if server == nil || strings.TrimSpace(server.URL) == "" {
			continue
		}

		name := strings.TrimSpace(server.Name)
		if name == "" {
			name = strings.TrimSpace(server.Description)
		}
		if name == "" {
			name = server.URL
		}

		doc := ServerDoc{
			Name:        name,
			URL:         server.URL,
			Description: server.Description,
			Variables:   convertServerVariables(server.Variables),
		}
		docs = append(docs, doc)
	}

	if len(docs) == 0 {
		return nil
	}

	return docs
}

func convertServerVariables(variables *orderedmap.Map[string, *v3.ServerVariable]) []ServerVariableDoc {
	if variables == nil || orderedmap.Len(variables) == 0 {
		return nil
	}

	docs := make([]ServerVariableDoc, 0, orderedmap.Len(variables))
	for pair := variables.First(); pair != nil; pair = pair.Next() {
		name := pair.Key()
		variable := pair.Value()
		if variable == nil {
			continue
		}
		docs = append(docs, ServerVariableDoc{
			Name:        name,
			Default:     variable.Default,
			Enum:        variable.Enum,
			Description: variable.Description,
		})
	}

	if len(docs) == 0 {
		return nil
	}

	return docs
}

func buildAuthDoc(opReqs, rootReqs []*base.SecurityRequirement, components *v3.Components) *AuthDoc {
	reqs := opReqs
	if len(reqs) == 0 {
		reqs = rootReqs
	}

	if len(reqs) == 0 {
		return nil
	}

	for _, req := range reqs {
		if req == nil {
			continue
		}

		if orderedmap.Len(req.Requirements) == 0 {
			return &AuthDoc{Required: false}
		}

		for pair := req.Requirements.First(); pair != nil; pair = pair.Next() {
			schemeName := pair.Key()
			auth := lookupSecurityDoc(components, schemeName)
			auth.Required = true
			return &auth
		}
	}

	return nil
}

// convertSecurityRequirements retains the complete OpenAPI security shape for
// SDK generators. buildAuthDoc intentionally remains a compact, legacy docs
// summary (it picks the first scheme), so it must not be used as this boundary.
func convertSecurityRequirements(opReqs, rootReqs []*base.SecurityRequirement) []SecurityRequirementDoc {
	reqs := opReqs
	if len(reqs) == 0 {
		reqs = rootReqs
	}
	if len(reqs) == 0 {
		return nil
	}

	docs := make([]SecurityRequirementDoc, 0, len(reqs))
	for _, req := range reqs {
		if req == nil {
			continue
		}
		schemes := make(map[string][]string, orderedmap.Len(req.Requirements))
		for pair := req.Requirements.First(); pair != nil; pair = pair.Next() {
			scopes := append([]string(nil), pair.Value()...)
			schemes[pair.Key()] = scopes
		}
		docs = append(docs, SecurityRequirementDoc{Schemes: schemes})
	}
	if len(docs) == 0 {
		return nil
	}
	return docs
}

func lookupSecurityDoc(components *v3.Components, name string) AuthDoc {
	auth := AuthDoc{Scheme: name}
	if components == nil || components.SecuritySchemes == nil {
		auth.Description = fallbackSecurityDescription(auth.Type, auth.HttpScheme, name)
		return auth
	}

	for pair := components.SecuritySchemes.First(); pair != nil; pair = pair.Next() {
		if pair.Key() != name {
			continue
		}
		scheme := pair.Value()
		if scheme == nil {
			auth.Description = fallbackSecurityDescription(auth.Type, auth.HttpScheme, name)
			return auth
		}

		auth.Type = scheme.Type
		auth.Name = scheme.Name
		auth.In = scheme.In
		auth.HttpScheme = scheme.Scheme
		auth.Description = strings.TrimSpace(scheme.Description)
		if auth.Description == "" {
			auth.Description = fallbackSecurityDescription(scheme.Type, scheme.Scheme, name)
		}
		return auth
	}

	auth.Description = fallbackSecurityDescription(auth.Type, auth.HttpScheme, name)
	return auth
}

func fallbackSecurityDescription(authType, httpScheme, name string) string {
	if authType == "http" && strings.EqualFold(httpScheme, "bearer") {
		return "Include an Authorization header with a Bearer token."
	}
	if authType == "apiKey" {
		return "Provide an API key for authentication."
	}
	return "Security scheme " + name
}

func mergeParameters(pathParams, opParams []ParameterDoc) []ParameterDoc {
	if len(pathParams) == 0 && len(opParams) == 0 {
		return nil
	}

	merged := append([]ParameterDoc{}, pathParams...)

	for _, param := range opParams {
		key := paramDocKey(param)
		replaced := false
		for i := range merged {
			if paramDocKey(merged[i]) == key {
				merged[i] = param
				replaced = true
				break
			}
		}
		if !replaced {
			merged = append(merged, param)
		}
	}

	if len(merged) == 0 {
		return nil
	}

	return merged
}

func paramDocKey(param ParameterDoc) string {
	return strings.ToLower(param.In) + ":" + strings.ToLower(param.Name)
}

func yamlNodeToInterface(node *yaml.Node) (any, error) {
	if node == nil {
		return nil, nil
	}

	var out any
	if err := node.Decode(&out); err != nil {
		return nil, err
	}

	return normalizeYAML(out), nil
}

func schemaProxyToInterface(proxy *base.SchemaProxy) (any, error) {
	if proxy == nil {
		return nil, nil
	}

	if proxy.IsReference() {
		ref := proxy.GetReference()
		schema := proxy.Schema()
		if schema == nil {
			if err := proxy.GetBuildError(); err != nil {
				return nil, err
			}
			return map[string]any{"$ref": ref}, nil
		}
		resolved, err := renderSchema(schema)
		if err != nil {
			return nil, err
		}
		return map[string]any{
			"$ref":     ref,
			"resolved": resolved,
		}, nil
	}

	schema := proxy.Schema()
	if schema == nil {
		if err := proxy.GetBuildError(); err != nil {
			return nil, err
		}
		return nil, nil
	}

	return renderSchema(schema)
}

func renderSchema(schema *base.Schema) (any, error) {
	rendered, err := schema.MarshalYAML()
	if err != nil {
		return nil, err
	}

	bytes, err := yaml.Marshal(rendered)
	if err != nil {
		return nil, err
	}

	var out any
	if err := yaml.Unmarshal(bytes, &out); err != nil {
		return nil, err
	}

	return normalizeYAML(out), nil
}

func normalizeYAML(value any) any {
	switch v := value.(type) {
	case map[interface{}]interface{}:
		out := make(map[string]any, len(v))
		for key, val := range v {
			out[fmt.Sprint(key)] = normalizeYAML(val)
		}
		return out
	case map[string]interface{}:
		out := make(map[string]any, len(v))
		for key, val := range v {
			out[key] = normalizeYAML(val)
		}
		return out
	case []interface{}:
		for i := range v {
			v[i] = normalizeYAML(v[i])
		}
		return v
	default:
		return v
	}
}

func schemaToFields(schema *base.Schema) []SchemaField {
	if schema == nil {
		return nil
	}

	if orderedmap.Len(schema.Properties) == 0 {
		primary := firstNonNullType(schema.Type)
		if primary == "array" && schema.Items != nil {
			if schema.Items.IsA() && schema.Items.A != nil {
				if child := schema.Items.A.Schema(); child != nil {
					return schemaToFields(child)
				}
			}
		}
		return nil
	}

	var fields []SchemaField
	for pair := schema.Properties.First(); pair != nil; pair = pair.Next() {
		name := pair.Key()
		proxy := pair.Value()

		field := SchemaField{Name: name}
		if proxy != nil {
			childSchema := proxy.Schema()
			if childSchema == nil {
				if err := proxy.GetBuildError(); err != nil {
					fmt.Printf("warning: unable to resolve schema for property %s: %v\n", name, err)
				}
			} else {
				field.Description = childSchema.Description
				field.Type = summarizeSchemaType(childSchema)
				field.Enum = enumValues(childSchema)

				primary := firstNonNullType(childSchema.Type)
				if primary == "object" {
					field.Children = schemaToFields(childSchema)
				} else if primary == "array" {
					if childSchema.Items != nil && childSchema.Items.IsA() && childSchema.Items.A != nil {
						subSchema := childSchema.Items.A.Schema()
						if subSchema != nil {
							field.Children = schemaToFields(subSchema)
						}
					}
				}
			}
		}

		fields = append(fields, field)
	}

	return fields
}

func summarizeSchemaType(schema *base.Schema) string {
	primary := firstNonNullType(schema.Type)
	if primary == "" {
		primary = "object"
	}

	switch primary {
	case "array":
		itemType := ""
		if schema.Items != nil {
			if schema.Items.IsA() && schema.Items.A != nil {
				if sub := schema.Items.A.Schema(); sub != nil {
					itemType = summarizeSchemaType(sub)
				}
			}
		}
		if itemType != "" {
			return itemType + "[]"
		}
		return "array"
	case "object":
		return "object"
	default:
		if len(schema.Enum) > 0 {
			return fmt.Sprintf("enum<%s>", primary)
		}
		if schema.Format != "" {
			return fmt.Sprintf("%s:%s", primary, schema.Format)
		}
		return primary
	}
}

func enumValues(schema *base.Schema) []string {
	if len(schema.Enum) == 0 {
		return nil
	}

	values := make([]string, 0, len(schema.Enum))
	for _, node := range schema.Enum {
		if node == nil {
			continue
		}
		val, err := yamlNodeToInterface(node)
		if err != nil {
			continue
		}
		switch v := val.(type) {
		case string:
			values = append(values, v)
		default:
			values = append(values, fmt.Sprintf("%v", v))
		}
	}

	if len(values) == 0 {
		return nil
	}

	return values
}

func writeJSON(path string, v any) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}

	buf := &bytes.Buffer{}
	encoder := json.NewEncoder(buf)
	encoder.SetEscapeHTML(false)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(v); err != nil {
		return err
	}

	if !strings.HasSuffix(path, ".json") {
		return fmt.Errorf("expected .json file, got %s", path)
	}

	return os.WriteFile(path, buf.Bytes(), 0o644)
}

func slugFor(path, method string) string {
	trimmed := strings.Trim(path, "/")
	if trimmed == "" {
		trimmed = "root"
	}

	var builder strings.Builder
	builder.Grow(len(trimmed) + len(method) + 1)
	builder.WriteString(method)
	builder.WriteRune('_')

	lastUnderscore := false

	for _, r := range trimmed {
		switch {
		case unicode.IsLetter(r) || unicode.IsDigit(r):
			builder.WriteRune(unicode.ToLower(r))
			lastUnderscore = false
		default:
			if !lastUnderscore {
				builder.WriteRune('_')
				lastUnderscore = true
			}
		}
	}

	slug := builder.String()
	slug = strings.Trim(slug, "_")
	if slug == "" {
		slug = method + "_root"
	}

	return slug
}
