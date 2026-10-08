package main

import (
	"fmt"
	compatible "github.com/goccy/go-yaml"
	yaml "go.yaml.in/yaml/v4"
	"regexp"
	"strings"
)

// Preserve original bytes for provenance. The fallback parses valid YAML which
// the primary scanner rejects (including leading tabs inside block content and
// YAML 1.2 Unicode separators), then presents equivalent JSON to libopenapi.
// Never repair indentation or replace scalar characters with different values.
func compatibleYAML(data []byte) ([]byte, error) {
	var node yaml.Node
	if err := yaml.Unmarshal(data, &node); err == nil {
		return data, nil
	}
	if err := rejectIndentationTabs(data); err != nil {
		return nil, err
	}
	normalized, err := compatible.YAMLToJSON(data)
	if err != nil {
		return nil, fmt.Errorf("parse YAML: %w", err)
	}
	if err := yaml.Unmarshal(normalized, &node); err != nil {
		return nil, err
	}
	return normalized, nil
}
func unmarshalDocument(data []byte, target any) error {
	if err := yaml.Unmarshal(data, target); err == nil {
		return nil
	}
	normalized, err := compatibleYAML(data)
	if err != nil {
		return err
	}
	return yaml.Unmarshal(normalized, target)
}

var blockScalarHeader = regexp.MustCompile(`(?:[:\-]\s+|^)[|>][+\-1-9]*\s*(?:#.*)?$`)

func rejectIndentationTabs(data []byte) error {
	blockIndent := -1
	for index, line := range strings.Split(string(data), "\n") {
		indent := len(line) - len(strings.TrimLeft(line, " "))
		rest := line[indent:]
		if strings.TrimSpace(line) == "" {
			continue
		}
		if blockIndent >= 0 && indent > blockIndent {
			continue
		}
		blockIndent = -1
		if strings.HasPrefix(rest, "\t") && !strings.HasPrefix(strings.TrimSpace(rest), "#") {
			return fmt.Errorf("tab in YAML indentation on line %d", index+1)
		}
		if blockScalarHeader.MatchString(rest) {
			blockIndent = indent
		}
	}
	return nil
}
