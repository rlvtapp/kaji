package main

import (
	"fmt"
	"reflect"

	"github.com/pb33f/libopenapi/datamodel/high/base"
	"github.com/pb33f/libopenapi/orderedmap"

	yaml "go.yaml.in/yaml/v4"
)

func extractExampleValue(example *base.Example) (any, error) {
	if example == nil {
		return nil, nil
	}

	if example.Value != nil {
		value, err := yamlNodeToInterface(example.Value)
		if err != nil {
			return nil, err
		}
		if value != nil {
			return value, nil
		}
	}

	if example.DataValue != nil {
		value, err := yamlNodeToInterface(example.DataValue)
		if err != nil {
			return nil, err
		}
		if value != nil {
			return value, nil
		}
	}

	if example.SerializedValue != "" {
		return example.SerializedValue, nil
	}

	if example.ExternalValue != "" {
		return example.ExternalValue, nil
	}

	return nil, nil
}

func generateExampleFromSchemaProxy(proxy *base.SchemaProxy) (any, error) {
	if proxy == nil {
		return nil, nil
	}

	schema := proxy.Schema()
	if schema == nil {
		if err := proxy.GetBuildError(); err != nil {
			return nil, err
		}
		if proxy.IsReference() {
			return map[string]any{"$ref": proxy.GetReference()}, nil
		}
		return nil, nil
	}

	return generateExampleFromSchema(schema)
}

func generateExampleFromSchema(schema *base.Schema) (any, error) {
	if schema == nil {
		return "<any>", nil
	}

	if example, ok, err := extractSchemaExample(schema); ok || err != nil {
		return example, err
	}

	if schema.Const != nil {
		if value, err := yamlNodeToInterface(schema.Const); err == nil && value != nil {
			return value, nil
		} else if err != nil {
			return nil, err
		}
	}

	if schema.Default != nil {
		if value, err := yamlNodeToInterface(schema.Default); err == nil && value != nil {
			return value, nil
		} else if err != nil {
			return nil, err
		}
	}

	if len(schema.Enum) > 0 {
		for _, node := range schema.Enum {
			if node == nil {
				continue
			}
			value, err := yamlNodeToInterface(node)
			if err != nil {
				return nil, err
			}
			if value != nil {
				return value, nil
			}
		}
	}

	if len(schema.OneOf) > 0 {
		return generateExampleFromSchemaProxy(schema.OneOf[0])
	}

	if len(schema.AnyOf) > 0 {
		return generateExampleFromSchemaProxy(schema.AnyOf[0])
	}

	primary := firstNonNullType(schema.Type)
	if primary == "" {
		primary = "object"
	}

	switch primary {
	case "object":
		result := map[string]any{}

		if len(schema.AllOf) > 0 {
			for _, proxy := range schema.AllOf {
				if proxy == nil {
					continue
				}
				value, err := generateExampleFromSchemaProxy(proxy)
				if err != nil {
					return nil, err
				}
				if subMap, ok := toStringAnyMap(value); ok {
					for k, v := range subMap {
						result[k] = v
					}
				}
			}
		}

		if orderedmap.Len(schema.Properties) > 0 {
			for pair := schema.Properties.First(); pair != nil; pair = pair.Next() {
				key := pair.Key()
				prop := pair.Value()
				var placeholder any = "<any>"
				if prop != nil {
					value, err := generateExampleFromSchemaProxy(prop)
					if err != nil {
						return nil, err
					}
					if value != nil {
						placeholder = value
					}
				}
				result[key] = placeholder
			}
		}

		if len(result) == 0 {
			return map[string]any{}, nil
		}
		return result, nil

	case "array":
		if schema.Items != nil {
			if schema.Items.IsA() && schema.Items.A != nil {
				value, err := generateExampleFromSchemaProxy(schema.Items.A)
				if err != nil {
					return nil, err
				}
				if value == nil {
					value = "<any>"
				}
				return []any{value}, nil
			}
			if schema.Items.IsB() {
				if schema.Items.B {
					return []any{"<any>"}, nil
				}
				return []any{}, nil
			}
		}
		return []any{"<any>"}, nil

	case "integer":
		return "<integer>", nil
	case "number":
		return "<number>", nil
	case "boolean":
		return "<boolean>", nil
	case "null":
		return nil, nil
	case "string":
		if schema.Format != "" {
			return fmt.Sprintf("<string:%s>", schema.Format), nil
		}
		return "<string>", nil
	default:
		return fmt.Sprintf("<%s>", primary), nil
	}
}

func extractSchemaExample(schema *base.Schema) (any, bool, error) {
	if schema == nil {
		return nil, false, nil
	}

	if value, ok, err := extractExampleField(schema, "Example"); ok || err != nil {
		return value, ok, err
	}

	if value, ok, err := extractExampleField(schema, "Examples"); ok || err != nil {
		return value, ok, err
	}

	return nil, false, nil
}

func extractExampleField(schema *base.Schema, fieldName string) (any, bool, error) {
	value := reflect.ValueOf(schema)
	if !value.IsValid() || value.IsNil() {
		return nil, false, nil
	}

	value = value.Elem()
	field := value.FieldByName(fieldName)
	if !field.IsValid() || !field.CanInterface() {
		return nil, false, nil
	}

	if field.Kind() == reflect.Pointer {
		if field.IsNil() {
			return nil, false, nil
		}
		return decodeExampleValue(field.Interface())
	}

	if field.Kind() == reflect.Slice {
		if field.Len() == 0 {
			return nil, false, nil
		}
		for i := 0; i < field.Len(); i++ {
			item := field.Index(i)
			if !item.IsValid() {
				continue
			}
			if item.Kind() == reflect.Pointer && item.IsNil() {
				continue
			}
			if value, ok, err := decodeExampleValue(item.Interface()); ok || err != nil {
				return value, ok, err
			}
		}
		return nil, false, nil
	}

	if field.IsZero() {
		return nil, false, nil
	}

	return decodeExampleValue(field.Interface())
}

func decodeExampleValue(value any) (any, bool, error) {
	if value == nil {
		return nil, false, nil
	}

	switch v := value.(type) {
	case *yaml.Node:
		decoded, err := yamlNodeToInterface(v)
		if err != nil {
			return nil, false, err
		}
		if decoded == nil {
			return nil, false, nil
		}
		return decoded, true, nil
	case yaml.Node:
		decoded, err := yamlNodeToInterface(&v)
		if err != nil {
			return nil, false, err
		}
		if decoded == nil {
			return nil, false, nil
		}
		return decoded, true, nil
	default:
		return value, true, nil
	}
}

func mergeExample(base, example any) any {
	if base == nil {
		if example == nil {
			return nil
		}
		return example
	}

	switch baseTyped := base.(type) {
	case map[string]any:
		exampleMap, ok := toStringAnyMap(example)
		if !ok {
			if example != nil {
				return example
			}
			clone := make(map[string]any, len(baseTyped))
			for k, v := range baseTyped {
				clone[k] = v
			}
			return clone
		}
		result := make(map[string]any, len(baseTyped))
		for k, v := range baseTyped {
			if ex, ok := exampleMap[k]; ok {
				result[k] = mergeExample(v, ex)
			} else {
				result[k] = v
			}
		}
		for k, v := range exampleMap {
			if _, ok := result[k]; !ok {
				result[k] = v
			}
		}
		return result

	case []any:
		exampleSlice, ok := toAnySlice(example)
		if ok && len(exampleSlice) > 0 {
			var placeholder any
			if len(baseTyped) > 0 {
				placeholder = baseTyped[0]
			}
			merged := make([]any, len(exampleSlice))
			for i, val := range exampleSlice {
				merged[i] = mergeExample(placeholder, val)
			}
			return merged
		}
		if example != nil {
			return example
		}
		clone := make([]any, len(baseTyped))
		copy(clone, baseTyped)
		return clone

	default:
		if example != nil {
			return example
		}
		return base
	}
}

func firstNonNullType(types []string) string {
	for _, t := range types {
		if t != "" && t != "null" {
			return t
		}
	}
	if len(types) > 0 {
		return types[0]
	}
	return ""
}

func toStringAnyMap(value any) (map[string]any, bool) {
	if value == nil {
		return nil, false
	}
	if m, ok := value.(map[string]any); ok {
		return m, true
	}
	if m, ok := value.(map[string]interface{}); ok {
		out := make(map[string]any, len(m))
		for k, v := range m {
			out[k] = v
		}
		return out, true
	}
	return nil, false
}

func toAnySlice(value any) ([]any, bool) {
	if value == nil {
		return nil, false
	}
	if slice, ok := value.([]any); ok {
		return slice, true
	}
	if slice, ok := value.([]interface{}); ok {
		result := make([]any, len(slice))
		for i, v := range slice {
			result[i] = v
		}
		return result, true
	}
	return nil, false
}
