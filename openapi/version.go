package main

import (
	"fmt"
	yaml "go.yaml.in/yaml/v4"
	"strings"
)

// Fail before touching artifacts rather than silently treating new operation
// or media constructs as an older contract. A version change is not conversion.
func validateDocumentVersion(data []byte) error {
	var header struct {
		OpenAPI string `yaml:"openapi"`
		Swagger string `yaml:"swagger"`
	}
	if err := yaml.Unmarshal(data, &header); err != nil {
		return fmt.Errorf("parse spec version: %w", err)
	}
	if header.OpenAPI != "" {
		if strings.HasPrefix(header.OpenAPI, "3.0.") || strings.HasPrefix(header.OpenAPI, "3.1.") {
			return nil
		}
		return fmt.Errorf("unsupported OpenAPI version %q: Kaji supports 3.0/3.1; OpenAPI 3.2 query/additionalOperations, querystring parameters and streaming itemSchema require compiler support, not changing the version string", header.OpenAPI)
	}
	if header.Swagger == "2.0" {
		return nil
	}
	return fmt.Errorf("specification must declare supported openapi (3.0/3.1) or swagger (2.0) version")
}
