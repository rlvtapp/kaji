package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"net/url"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"unicode"

	yaml "go.yaml.in/yaml/v4"
)

// Normalize local references at the syntax boundary. Schema targets are lifted
// to named root components rather than expanded, so recursive types stay finite.
// Other Reference Objects are inlined with an explicit non-schema cycle guard.
func bundleLocalSources(source *sourceClosure, specPath string) ([]byte, error) {
	rootPath, err := filepath.Abs(specPath)
	if err != nil {
		return nil, err
	}
	rootPath = filepath.Clean(rootPath)
	if source.Root != "" {
		rootPath = source.Root
	}
	documents := map[string]*yaml.Node{}
	for path, data := range source.Documents {
		var document yaml.Node
		if err := yaml.Unmarshal(data, &document); err != nil {
			return nil, err
		}
		documents[path] = document.Content[0]
	}
	root := documents[rootPath]
	if root == nil || root.Kind != yaml.MappingNode {
		return nil, fmt.Errorf("OpenAPI root must be an object")
	}
	lifted := map[string]string{}
	schemas := map[string]*yaml.Node{}
	active := map[string]bool{}
	used := map[string]bool{}
	if components := mappingValue(root, "components"); components != nil {
		if catalog := mappingValue(components, "schemas"); catalog != nil {
			for i := 0; i+1 < len(catalog.Content); i += 2 {
				used[catalog.Content[i].Value] = true
			}
		}
	}
	locate := func(origin, ref string) (string, string, *yaml.Node, error) {
		uri, err := url.Parse(ref)
		if err != nil {
			return "", "", nil, err
		}
		resolutionBase := origin
		if base, exists := source.Bases[origin]; exists {
			resolutionBase = base
		}
		path, fragment, err := resolveSourceReference(resolutionBase, ref)
		if identity, exists := source.Identities[path]; exists {
			path = identity
		}
		if err != nil {
			return "", "", nil, err
		}
		uri.Fragment = fragment
		node := documents[path]
		if node == nil {
			return "", "", nil, fmt.Errorf("reference document %s is outside the collected local closure", path)
		}
		if uri.Fragment != "" {
			if !strings.HasPrefix(uri.Fragment, "/") {
				return "", "", nil, fmt.Errorf("external named anchors are not supported: %q; use a JSON Pointer", ref)
			}
			for _, part := range strings.Split(strings.TrimPrefix(uri.Fragment, "/"), "/") {
				part = strings.ReplaceAll(strings.ReplaceAll(part, "~1", "/"), "~0", "~")
				node = pointerValue(node, part)
				if node == nil {
					return "", "", nil, fmt.Errorf("local reference %q from %s has no JSON Pointer target", ref, origin)
				}
			}
		}
		return path, uri.Fragment, node, nil
	}
	var normalize func(*yaml.Node, string, bool, bool) (*yaml.Node, error)
	liftSchema := func(path, fragment string, target *yaml.Node) (string, error) {
		locator := path + "#" + fragment
		name, exists := lifted[locator]
		if !exists {
			hint := strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
			if fragment != "" {
				parts := strings.Split(fragment, "/")
				hint = parts[len(parts)-1]
			}
			var label strings.Builder
			for _, r := range hint {
				if unicode.IsLetter(r) || unicode.IsDigit(r) || r == '_' {
					label.WriteRune(r)
				} else {
					label.WriteByte('_')
				}
			}
			relative := path
			if !isRemoteDocument(path) {
				relative, _ = filepath.Rel(source.Base, path)
			}
			digest := sha256.Sum256([]byte(filepath.ToSlash(relative) + "#" + fragment))
			name = "Local_" + label.String() + "_" + hex.EncodeToString(digest[:6])
			for used[name] {
				name += "_"
			}
			used[name] = true
			lifted[locator] = name
			// Register the name before traversing its contents: a recursive target
			// becomes a reference to this component instead of an infinite tree.
			normalized, err := normalize(target, path, true, false)
			if err != nil {
				return "", err
			}
			schemas[name] = normalized
		}
		return name, nil
	}

	normalizedNodes, depth := 0, 0
	normalize = func(node *yaml.Node, origin string, isSchema, schemaEntries bool) (*yaml.Node, error) {
		normalizedNodes++
		depth++
		defer func() { depth-- }()
		if normalizedNodes > 2_000_000 || depth > 256 {
			return nil, fmt.Errorf("local-reference normalization exceeds bounded node/depth limits")
		}
		if node == nil {
			return nil, nil
		}
		if node.Kind == yaml.MappingNode {
			reference := mappingValue(node, "$ref")
			if reference != nil && reference.Kind == yaml.ScalarNode {
				uri, err := url.Parse(reference.Value)
				if err != nil {
					return nil, err
				}
				// Root-local references already have the correct scope and are normalized
				// when their component declaration is visited.
				if origin != rootPath || uri.Path != "" || uri.Host != "" {
					path, fragment, target, err := locate(origin, reference.Value)
					if err != nil {
						return nil, err
					}
					locator := path + "#" + fragment
					if isSchema {
						name, err := liftSchema(path, fragment, target)
						if err != nil {
							return nil, err
						}
						result := *node
						result.Content = nil
						for i := 0; i+1 < len(node.Content); i += 2 {
							key := cloneScalar(node.Content[i])
							value := node.Content[i+1]
							if key.Value == "$ref" {
								value = &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: "#/components/schemas/" + name}
							} else {
								value, err = normalize(value, origin, true, false)
								if err != nil {
									return nil, err
								}
							}
							result.Content = append(result.Content, key, value)
						}
						return &result, nil
					}
					if active[locator] {
						return nil, fmt.Errorf("unsupported circular non-schema reference %q from %s", reference.Value, origin)
					}
					active[locator] = true
					normalized, err := normalize(target, path, false, false)
					delete(active, locator)
					if err != nil {
						return nil, err
					}
					if normalized.Kind == yaml.MappingNode {
						// The permitted summary/description Reference Object siblings override
						// the target annotation; other siblings follow existing object semantics.
						for i := 0; i+1 < len(node.Content); i += 2 {
							key := node.Content[i]
							if key.Value == "$ref" {
								continue
							}
							value, err := normalize(node.Content[i+1], origin, false, false)
							if err != nil {
								return nil, err
							}
							setMappingValue(normalized, key.Value, value)
						}
					} else if len(node.Content) > 2 {
						return nil, fmt.Errorf("non-object local reference %q cannot carry siblings", reference.Value)
					}
					return normalized, nil
				}
			}
		}
		result := *node
		result.Content = nil
		if node.Kind == yaml.MappingNode {
			for i := 0; i+1 < len(node.Content); i += 2 {
				key, value := node.Content[i], node.Content[i+1]
				nextSchema := isSchema || schemaEntries || key.Value == "schema" || key.Value == "itemSchema"
				nextEntries := key.Value == "schemas" && !isSchema || isSchema && (key.Value == "properties" || key.Value == "patternProperties" || key.Value == "$defs" || key.Value == "definitions")
				if isSchema && !schemaEntries && (key.Value == "default" || key.Value == "const" || key.Value == "enum" || key.Value == "example" || key.Value == "examples") {
					result.Content = append(result.Content, cloneScalar(key), cloneLiteral(value))
					continue
				}
				normalized, err := normalize(value, origin, nextSchema, nextEntries)
				if isSchema && !schemaEntries && key.Value == "discriminator" && err == nil {
					mappings := mappingValue(normalized, "mapping")
					if mappings != nil {
						for j := 0; j+1 < len(mappings.Content); j += 2 {
							mapping := mappings.Content[j+1]
							if mapping.Kind != yaml.ScalarNode {
								return nil, fmt.Errorf("discriminator mapping must be a string")
							}
							uri, parseErr := url.Parse(mapping.Value)
							if parseErr != nil {
								return nil, parseErr
							}
							if origin == rootPath && uri.Path == "" && uri.Host == "" && uri.Scheme == "" {
								continue
							}
							ref := mapping.Value
							if !strings.ContainsAny(ref, "/#.") {
								if origin == rootPath {
									continue
								}
								ref = "#/" + ref
							}
							path, fragment, target, locateErr := locate(origin, ref)
							if locateErr != nil {
								return nil, locateErr
							}
							name, liftErr := liftSchema(path, fragment, target)
							if liftErr != nil {
								return nil, liftErr
							}
							mapping.Value = "#/components/schemas/" + name
						}
					}
				}
				if err != nil {
					return nil, err
				}
				result.Content = append(result.Content, cloneScalar(key), normalized)
			}
		} else {
			for _, child := range node.Content {
				normalized, err := normalize(child, origin, isSchema, false)
				if err != nil {
					return nil, err
				}
				result.Content = append(result.Content, normalized)
			}
		}
		return &result, nil
	}
	normalized, err := normalize(root, rootPath, false, false)
	if err != nil {
		return nil, err
	}
	components := mappingValue(normalized, "components")
	if components == nil {
		components = &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map"}
		setMappingValue(normalized, "components", components)
	}
	catalog := mappingValue(components, "schemas")
	if catalog == nil {
		catalog = &yaml.Node{Kind: yaml.MappingNode, Tag: "!!map"}
		setMappingValue(components, "schemas", catalog)
	}
	// Stable ordering keeps the normalized contract and generated output reproducible.
	for _, name := range sortedSchemaNames(schemas) {
		setMappingValue(catalog, name, schemas[name])
	}
	return yaml.Marshal(normalized)
}

func mappingValue(node *yaml.Node, key string) *yaml.Node {
	if node == nil || node.Kind != yaml.MappingNode {
		return nil
	}
	for i := 0; i+1 < len(node.Content); i += 2 {
		if node.Content[i].Value == key {
			return node.Content[i+1]
		}
	}
	return nil
}
func setMappingValue(node *yaml.Node, key string, value *yaml.Node) {
	for i := 0; i+1 < len(node.Content); i += 2 {
		if node.Content[i].Value == key {
			node.Content[i+1] = value
			return
		}
	}
	node.Content = append(node.Content, &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: key}, value)
}
func cloneScalar(node *yaml.Node) *yaml.Node { value := *node; return &value }

func sortedSchemaNames(schemas map[string]*yaml.Node) []string {
	names := make([]string, 0, len(schemas))
	for name := range schemas {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func pointerValue(node *yaml.Node, key string) *yaml.Node {
	if node != nil && node.Kind == yaml.SequenceNode {
		index, err := strconv.Atoi(key)
		if err == nil && index >= 0 && index < len(node.Content) {
			return node.Content[index]
		}
		return nil
	}
	return mappingValue(node, key)
}
func cloneLiteral(node *yaml.Node) *yaml.Node {
	if node == nil {
		return nil
	}
	value := *node
	value.Content = nil
	for _, child := range node.Content {
		value.Content = append(value.Content, cloneLiteral(child))
	}
	return &value
}
