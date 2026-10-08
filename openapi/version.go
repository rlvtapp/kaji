package main

import (
	"fmt"
	"strings"
)

// Fail before touching artifacts rather than silently treating new operation
// or media constructs as an older contract. A version change is not conversion.
func validateDocumentVersion(data []byte) error {
	var header struct {
		OpenAPI string `yaml:"openapi"`
		Swagger string `yaml:"swagger"`
	}
	if err := unmarshalDocument(data, &header); err != nil {
		return fmt.Errorf("parse spec version: %w", err)
	}
	if header.OpenAPI != "" {
		if strings.HasPrefix(header.OpenAPI, "3.0.") || strings.HasPrefix(header.OpenAPI, "3.1.") || strings.HasPrefix(header.OpenAPI, "3.2.") {
			return nil
		}
		return fmt.Errorf("unsupported OpenAPI version %q: Kaji supports 3.0/3.1/3.2; newer versions require compiler support, not changing the version string", header.OpenAPI)
	}
	if header.Swagger == "2.0" {
		return nil
	}
	return fmt.Errorf("specification must declare supported openapi (3.0/3.1/3.2) or swagger (2.0) version")
}
