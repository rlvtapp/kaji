package main

import (
	"fmt"
	"sort"
	"strings"

	v3 "github.com/pb33f/libopenapi/datamodel/high/v3"
	"github.com/pb33f/libopenapi/orderedmap"
)

// Validate resolved model objects before cleaning any existing artifacts. New
// wire representations must never disappear merely because parsing succeeds.
func validateCompilerCapabilities(spec *v3.Document) error {
	checkMedia := func(content *orderedmap.Map[string, *v3.MediaType], where string) error {
		if content == nil {
			return nil
		}
		for p := content.First(); p != nil; p = p.Next() {
			mt := p.Value()
			if mt == nil {
				continue
			}
			if _, err := convertMediaDefinition(p.Key(), mt); err != nil {
				return fmt.Errorf("%s content %s: %w", where, p.Key(), err)
			}
		}
		return nil
	}
	checkParams := func(params []*v3.Parameter, where string) error {
		whole, named := 0, 0
		for _, p := range params {
			if p == nil {
				continue
			}
			if p.In == "query" {
				named++
			}
			if p.In == "querystring" {
				whole++
				if p.Schema != nil || p.Content == nil || orderedmap.Len(p.Content) != 1 {
					return fmt.Errorf("%s: querystring requires exactly one content entry and no schema", where)
				}
			}
		}
		if whole > 1 || whole > 0 && named > 0 {
			return fmt.Errorf("%s: querystring cannot coexist with another querystring or query parameter", where)
		}
		return nil
	}
	checkPaths := func(items *orderedmap.Map[string, *v3.PathItem], kind string) error {
		if items == nil {
			return nil
		}
		for p := items.First(); p != nil; p = p.Next() {
			item := p.Value()
			if item == nil {
				continue
			}
			where := kind + " " + p.Key()
			if err := checkParams(item.Parameters, where); err != nil {
				return err
			}
			if err := validateAdditionalMethods(item); err != nil {
				return fmt.Errorf("%s: %w", where, err)
			}
			ops := compilerOperations(item)
			if ops == nil {
				continue
			}
			for op := ops.First(); op != nil; op = op.Next() {
				value := op.Value()
				if value == nil {
					continue
				}
				label := where + " " + strings.ToUpper(op.Key())
				if err := checkParams(mergeParameterObjects(item.Parameters, value.Parameters), label); err != nil {
					return err
				}
				if value.RequestBody != nil {
					if err := checkMedia(value.RequestBody.Content, label+" request"); err != nil {
						return err
					}
				}
				if value.Responses != nil && value.Responses.Codes != nil {
					for response := value.Responses.Codes.First(); response != nil; response = response.Next() {
						if response.Value() != nil {
							if err := checkMedia(response.Value().Content, label+" response "+response.Key()); err != nil {
								return err
							}
						}
					}
				}
				if value.Responses != nil && value.Responses.Default != nil {
					if err := checkMedia(value.Responses.Default.Content, label+" default response"); err != nil {
						return err
					}
				}
			}
		}
		return nil
	}
	if spec.Paths != nil {
		if err := checkPaths(spec.Paths.PathItems, "path"); err != nil {
			return err
		}
	}
	return checkPaths(spec.Webhooks, "webhook")
}

func validateAdditionalMethods(item *v3.PathItem) error {
	if low := item.GoLow(); low != nil && low.RootNode != nil {
		additional := mappingValue(low.RootNode, "additionalOperations")
		if additional != nil {
			for i := 0; i+1 < len(additional.Content); i += 2 {
				if err := validateHTTPMethodToken(additional.Content[i].Value); err != nil {
					return err
				}
			}
		}
	}
	if item.AdditionalOperations == nil {
		return nil
	}
	for pair := item.AdditionalOperations.First(); pair != nil; pair = pair.Next() {
		method := pair.Key()
		if err := validateHTTPMethodToken(method); err != nil {
			return err
		}
	}
	return nil
}

func validateHTTPMethodToken(method string) error {
	if len(method) == 0 || len(method) > 256 {
		return fmt.Errorf("additionalOperations HTTP method must contain 1..256 ASCII token bytes")
	}
	for _, r := range method {
		if !(r >= 'A' && r <= 'Z' || r >= 'a' && r <= 'z' || r >= '0' && r <= '9' || strings.ContainsRune("!#$%&'*+-.^_`|~", r)) {
			return fmt.Errorf("additionalOperations has invalid HTTP method token")
		}
	}
	switch strings.ToUpper(method) {
	case "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE", "QUERY":
		return fmt.Errorf("additionalOperations must not shadow standard HTTP method %s", method)
	}
	return nil
}

// libopenapi 0.38.7 omits the low additionalOperations KeyNode and therefore
// high-level IsEmpty/GetOperations can lose custom methods. Read its populated
// low values explicitly, retaining source-line order alongside standard methods.
func compilerOperations(item *v3.PathItem) *orderedmap.Map[string, *v3.Operation] {
	result := item.GetOperations()
	if low := item.GoLow(); low != nil && low.AdditionalOperations.Value != nil {
		if item.AdditionalOperations == nil {
			item.AdditionalOperations = orderedmap.New[string, *v3.Operation]()
		}
		for key, value := range low.AdditionalOperations.Value.FromOldest() {
			if value.Value != nil {
				item.AdditionalOperations.Set(key.Value, v3.NewOperation(value.Value))
			}
		}
	}
	if item.AdditionalOperations != nil {
		for pair := item.AdditionalOperations.First(); pair != nil; pair = pair.Next() {
			result.Set(pair.Key(), pair.Value())
		}
	}
	type entry struct {
		key       string
		operation *v3.Operation
		line      int
		column    int
	}
	entries := []entry{}
	for pair := result.First(); pair != nil; pair = pair.Next() {
		line, column := 0, 0
		if pair.Value() != nil && pair.Value().GoLow() != nil && pair.Value().GoLow().KeyNode != nil {
			line = pair.Value().GoLow().KeyNode.Line
			column = pair.Value().GoLow().KeyNode.Column
		}
		entries = append(entries, entry{pair.Key(), pair.Value(), line, column})
	}
	sort.SliceStable(entries, func(i, j int) bool {
		if entries[i].line == entries[j].line {
			return entries[i].column < entries[j].column
		}
		return entries[i].line < entries[j].line
	})
	sorted := orderedmap.New[string, *v3.Operation]()
	for _, entry := range entries {
		sorted.Set(entry.key, entry.operation)
	}
	return sorted
}

func mergeParameterObjects(path, operation []*v3.Parameter) []*v3.Parameter {
	result := append([]*v3.Parameter{}, path...)
	for _, value := range operation {
		if value == nil {
			continue
		}
		found := false
		for i, existing := range result {
			if existing != nil && existing.In == value.In && existing.Name == value.Name {
				result[i] = value
				found = true
				break
			}
		}
		if !found {
			result = append(result, value)
		}
	}
	return result
}
