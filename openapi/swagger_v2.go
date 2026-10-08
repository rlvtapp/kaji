package main

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/cespare/xxhash/v2"
	"github.com/pb33f/libopenapi/datamodel/high/base"
	v2 "github.com/pb33f/libopenapi/datamodel/high/v2"
	"github.com/pb33f/libopenapi/orderedmap"
)

// compileSwaggerV2 normalizes Swagger's legacy model into exactly the same
// JSON artifact boundary used by the OpenAPI 3 compiler. Rust generators do
// not need a second source-format branch.
func compileSwaggerV2(spec *v2.Swagger, outDir string) (int, error) {
	if spec == nil {
		return 0, fmt.Errorf("Swagger model is nil")
	}
	if err := os.MkdirAll(outDir, 0o755); err != nil {
		return 0, fmt.Errorf("create output directory: %w", err)
	}
	operationsDir := filepath.Join(outDir, "operations")
	if err := os.RemoveAll(operationsDir); err != nil && !os.IsNotExist(err) {
		return 0, fmt.Errorf("cleanup operations dir: %w", err)
	}
	if err := os.MkdirAll(operationsDir, 0o755); err != nil {
		return 0, fmt.Errorf("create operations dir: %w", err)
	}

	schemas, err := collectSwaggerV2Schemas(spec.Definitions)
	if err != nil {
		return 0, fmt.Errorf("convert Swagger definitions: %w", err)
	}
	if err := writeJSON(filepath.Join(outDir, "schemas.json"), ComponentSchemasDoc{Schemas: schemas}); err != nil {
		return 0, fmt.Errorf("write component schemas: %w", err)
	}
	if err := writeJSON(filepath.Join(outDir, "security-schemes.json"), SecuritySchemesDoc{Schemes: collectSwaggerV2SecuritySchemes(spec.SecurityDefinitions)}); err != nil {
		return 0, fmt.Errorf("write security schemes: %w", err)
	}

	index := make(map[string]string)
	order := make([]string, 0)
	count, err := collectSwaggerV2Operations(spec, outDir, index, &order)
	if err != nil {
		return 0, err
	}
	if err := writeJSON(filepath.Join(outDir, "operations.json"), index); err != nil {
		return 0, fmt.Errorf("write operation index: %w", err)
	}
	if err := writeJSON(filepath.Join(outDir, "operations-order.json"), order); err != nil {
		return 0, fmt.Errorf("write operation order: %w", err)
	}
	return count, nil
}

func collectSwaggerV2Schemas(definitions *v2.Definitions) ([]ComponentSchemaDoc, error) {
	if definitions == nil || definitions.Definitions == nil || orderedmap.Len(definitions.Definitions) == 0 {
		return []ComponentSchemaDoc{}, nil
	}
	out := make([]ComponentSchemaDoc, 0, orderedmap.Len(definitions.Definitions))
	for pair := definitions.Definitions.First(); pair != nil; pair = pair.Next() {
		schema, err := schemaProxyToInterface(pair.Value())
		if err != nil {
			return nil, fmt.Errorf("render definition %q: %w", pair.Key(), err)
		}
		if schema == nil {
			return nil, fmt.Errorf("definition %q resolved to nil", pair.Key())
		}
		out = append(out, ComponentSchemaDoc{Name: pair.Key(), Schema: schema})
	}
	return out, nil
}

func collectSwaggerV2SecuritySchemes(definitions *v2.SecurityDefinitions) []SecuritySchemeDoc {
	if definitions == nil || definitions.Definitions == nil || orderedmap.Len(definitions.Definitions) == 0 {
		return []SecuritySchemeDoc{}
	}
	out := make([]SecuritySchemeDoc, 0, orderedmap.Len(definitions.Definitions))
	for pair := definitions.Definitions.First(); pair != nil; pair = pair.Next() {
		scheme := pair.Value()
		if scheme == nil {
			continue
		}
		doc := SecuritySchemeDoc{Name: pair.Key(), Type: scheme.Type, Description: scheme.Description}
		switch scheme.Type {
		case "basic":
			doc.Type = "http"
			doc.HTTPScheme = "basic"
		case "apiKey":
			doc.APIKeyName = scheme.Name
			doc.APIKeyIn = scheme.In
		case "oauth2":
			flow := scheme.Flow
			switch flow {
			case "application":
				flow = "clientCredentials"
			case "accessCode":
				flow = "authorizationCode"
			}
			var scopes map[string]string
			if scheme.Scopes != nil && scheme.Scopes.Values != nil {
				scopes = make(map[string]string, orderedmap.Len(scheme.Scopes.Values))
				for scope := scheme.Scopes.Values.First(); scope != nil; scope = scope.Next() {
					scopes[scope.Key()] = scope.Value()
				}
			}
			doc.OAuthFlows = []OAuthFlowDoc{{
				Type:             flow,
				AuthorizationURL: scheme.AuthorizationUrl,
				TokenURL:         scheme.TokenUrl,
				Scopes:           scopes,
			}}
		}
		out = append(out, doc)
	}
	return out
}

func collectSwaggerV2Operations(spec *v2.Swagger, outDir string, index map[string]string, order *[]string) (int, error) {
	if spec.Paths == nil || spec.Paths.PathItems == nil {
		return 0, nil
	}
	count := 0
	usedSlugs := make(map[string]string)
	for pair := spec.Paths.PathItems.First(); pair != nil; pair = pair.Next() {
		path, item := pair.Key(), pair.Value()
		if item == nil || item.GetOperations() == nil {
			continue
		}
		for operationPair := item.GetOperations().First(); operationPair != nil; operationPair = operationPair.Next() {
			method, operation := strings.ToLower(operationPair.Key()), operationPair.Value()
			if operation == nil {
				continue
			}
			rawParams := mergeSwaggerV2Parameters(item.Parameters, operation.Parameters)
			parameters, err := convertSwaggerV2Parameters(rawParams)
			if err != nil {
				return 0, fmt.Errorf("convert Swagger parameters (%s %s): %w", method, path, err)
			}
			consumes := operation.Consumes
			if len(consumes) == 0 {
				consumes = spec.Consumes
			}
			requestBody, err := convertSwaggerV2RequestBody(rawParams, consumes)
			if err != nil {
				return 0, fmt.Errorf("convert Swagger request body (%s %s): %w", method, path, err)
			}
			produces := operation.Produces
			if len(produces) == 0 {
				produces = spec.Produces
			}
			responses, err := convertSwaggerV2Responses(operation.Responses, produces)
			if err != nil {
				return 0, fmt.Errorf("convert Swagger responses (%s %s): %w", method, path, err)
			}
			schemes := operation.Schemes
			if len(schemes) == 0 {
				schemes = spec.Schemes
			}
			deprecated := operation.Deprecated
			opDoc := OperationDoc{
				Kind:                 "operation",
				Name:                 strings.ToUpper(method) + " " + path,
				OperationID:          operation.OperationId,
				Path:                 path,
				Method:               strings.ToUpper(method),
				Summary:              operation.Summary,
				Description:          operation.Description,
				Deprecated:           deprecated,
				Hidden:               extensionBool(operation.Extensions, "x-hidden"),
				SecurityRequirements: convertSecurityRequirements(operation.Security, spec.Security),
				Servers:              swaggerV2Servers(spec.Host, spec.BasePath, schemes),
				Parameters:           parameters,
				RequestBody:          requestBody,
				Responses:            responses,
				Extensions:           convertExtensions(operation.Extensions),
			}
			slug := slugFor(path, method)
			identity := method + " " + path
			if previous, ok := usedSlugs[slug]; ok && previous != identity {
				slug += fmt.Sprintf("_%016x", xxhash.Sum64String(identity))
			}
			if previous, ok := usedSlugs[slug]; ok && previous != identity {
				return 0, fmt.Errorf("operation filename collision: %s and %s", previous, identity)
			}
			usedSlugs[slug] = identity
			if err := writeJSON(filepath.Join(outDir, "operations", slug+".json"), opDoc); err != nil {
				return 0, fmt.Errorf("write Swagger operation (%s %s): %w", method, path, err)
			}
			key := strings.ToUpper(method) + " " + path
			index[key] = slug + ".json"
			*order = append(*order, key)
			count++
		}
	}
	return count, nil
}

func mergeSwaggerV2Parameters(pathParameters, operationParameters []*v2.Parameter) []*v2.Parameter {
	var merged []*v2.Parameter
	positions := make(map[string]int)
	for _, parameters := range [][]*v2.Parameter{pathParameters, operationParameters} {
		for _, parameter := range parameters {
			if parameter == nil {
				continue
			}
			key := paramDocKey(ParameterDoc{Name: parameter.Name, In: parameter.In})
			if index, exists := positions[key]; exists {
				merged[index] = parameter
			} else {
				positions[key] = len(merged)
				merged = append(merged, parameter)
			}
		}
	}
	return merged
}

func convertSwaggerV2Parameters(parameters []*v2.Parameter) ([]ParameterDoc, error) {
	var out []ParameterDoc
	for _, parameter := range parameters {
		if parameter == nil || parameter.In == "body" || parameter.In == "formData" {
			continue
		}
		schema, err := swaggerV2ParameterSchema(parameter)
		if err != nil {
			return nil, err
		}
		style, explode := swaggerV2ParameterStyle(parameter)
		required := parameter.In == "path" || (parameter.Required != nil && *parameter.Required)
		out = append(out, ParameterDoc{
			Name: parameter.Name, In: parameter.In, Description: parameter.Description,
			Required: required, Style: style, Explode: explode, Schema: schema,
		})
	}
	return out, nil
}

func swaggerV2ParameterStyle(parameter *v2.Parameter) (string, *bool) {
	if parameter.CollectionFormat == "" {
		return "", nil
	}
	switch parameter.CollectionFormat {
	case "multi":
		value := true
		return "form", &value
	case "pipes":
		value := false
		return "pipeDelimited", &value
	case "ssv":
		value := false
		return "spaceDelimited", &value
	default: // csv is Swagger's default.
		value := false
		return "form", &value
	}
}

func swaggerV2ParameterSchema(parameter *v2.Parameter) (any, error) {
	if parameter.Schema != nil {
		return schemaProxyToInterface(parameter.Schema)
	}
	schema := map[string]any{}
	if parameter.Type != "" {
		schema["type"] = parameter.Type
	}
	if parameter.Format != "" {
		schema["format"] = parameter.Format
	}
	if parameter.Items != nil {
		items := map[string]any{}
		if parameter.Items.Type != "" {
			items["type"] = parameter.Items.Type
		}
		if parameter.Items.Format != "" {
			items["format"] = parameter.Items.Format
		}
		schema["items"] = items
	}
	if len(schema) == 0 {
		return nil, nil
	}
	return schema, nil
}

func convertSwaggerV2RequestBody(parameters []*v2.Parameter, consumes []string) (*BodyDoc, error) {
	var body *v2.Parameter
	forms := make([]*v2.Parameter, 0)
	for _, parameter := range parameters {
		if parameter == nil {
			continue
		}
		switch parameter.In {
		case "body":
			body = parameter
		case "formData":
			forms = append(forms, parameter)
		}
	}
	if body == nil && len(forms) == 0 {
		return nil, nil
	}
	if len(consumes) == 0 {
		consumes = []string{"application/json"}
	}
	var schema any
	var fields []SchemaField
	description := ""
	required := false
	if body != nil {
		var err error
		schema, err = swaggerV2ParameterSchema(body)
		if err != nil {
			return nil, err
		}
		description = body.Description
		required = body.Required != nil && *body.Required
		if body.Schema != nil && body.Schema.Schema() != nil {
			fields = schemaToFields(body.Schema.Schema())
		}
	} else {
		properties := map[string]any{}
		requiredNames := make([]string, 0)
		for _, parameter := range forms {
			value, err := swaggerV2ParameterSchema(parameter)
			if err != nil {
				return nil, err
			}
			properties[parameter.Name] = value
			if parameter.Required != nil && *parameter.Required {
				requiredNames = append(requiredNames, parameter.Name)
			}
		}
		schema = map[string]any{"type": "object", "properties": properties}
		if len(requiredNames) > 0 {
			schema.(map[string]any)["required"] = requiredNames
		}
		required = len(requiredNames) > 0
	}
	mediaTypes := make([]MediaTypeDoc, 0, len(consumes))
	for _, contentType := range consumes {
		mediaTypes = append(mediaTypes, MediaTypeDoc{ContentType: contentType, Schema: fields, SchemaDefinition: schema})
	}
	return &BodyDoc{Description: description, Required: required, MediaTypes: mediaTypes}, nil
}

func convertSwaggerV2Responses(responses *v2.Responses, produces []string) ([]ResponseDoc, error) {
	if responses == nil {
		return nil, nil
	}
	var out []ResponseDoc
	if responses.Codes != nil {
		for pair := responses.Codes.First(); pair != nil; pair = pair.Next() {
			docs, err := convertSwaggerV2Response(pair.Key(), pair.Value(), produces)
			if err != nil {
				return nil, err
			}
			out = append(out, docs...)
		}
	}
	if responses.Default != nil {
		docs, err := convertSwaggerV2Response("default", responses.Default, produces)
		if err != nil {
			return nil, err
		}
		out = append(out, docs...)
	}
	return out, nil
}

func convertSwaggerV2Response(code string, response *v2.Response, produces []string) ([]ResponseDoc, error) {
	if response == nil {
		return nil, nil
	}
	if response.Schema == nil {
		return []ResponseDoc{{Code: code, Description: response.Description}}, nil
	}
	schema, err := schemaProxyToInterface(response.Schema)
	if err != nil {
		return nil, err
	}
	var fields []SchemaField
	if concrete := response.Schema.Schema(); concrete != nil {
		fields = schemaToFields(concrete)
	}
	if len(produces) == 0 {
		produces = []string{"application/json"}
	}
	out := make([]ResponseDoc, 0, len(produces))
	for _, contentType := range produces {
		out = append(out, ResponseDoc{Code: code, Description: response.Description, ContentType: contentType, Schema: fields, SchemaDefinition: schema, ExampleJSON: swaggerV2ResponseExample(response, contentType)})
	}
	return out, nil
}

func swaggerV2ResponseExample(response *v2.Response, contentType string) string {
	if response.Examples == nil || response.Examples.Values == nil {
		return ""
	}
	value := response.Examples.Values.GetOrZero(contentType)
	if value == nil {
		return ""
	}
	example, err := yamlNodeToInterface(value)
	if err != nil {
		return ""
	}
	return formatExampleJSON(example)
}

func swaggerV2Servers(host, basePath string, schemes []string) []ServerDoc {
	if strings.TrimSpace(host) == "" {
		return nil
	}
	if basePath == "" {
		basePath = "/"
	}
	if !strings.HasPrefix(basePath, "/") {
		basePath = "/" + basePath
	}
	schemes = append([]string(nil), schemes...)
	sort.Strings(schemes)
	docs := make([]ServerDoc, 0, len(schemes))
	for _, scheme := range schemes {
		if scheme == "" {
			continue
		}
		url := scheme + "://" + host + basePath
		docs = append(docs, ServerDoc{Name: url, URL: url})
	}
	return docs
}

// Keep the compiler's base package import in this file explicit: Swagger
// security requirements share the same normalized representation as OpenAPI 3.
var _ = base.SecurityRequirement{}
