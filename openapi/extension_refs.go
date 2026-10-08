package main

import (
	"fmt"
	"go.yaml.in/yaml/v4"
	"strings"
)

// Extension payloads are arbitrary data, not OpenAPI reference objects. Keep
// their nodes intact while preventing the model index from resolving their keys.
func shieldExtensionReferences(root *yaml.Node) func() {
	var keys []*yaml.Node
	var walk func(*yaml.Node, bool, bool)
	walk = func(node *yaml.Node, opaque bool, entries bool) {
		if node == nil {
			return
		}
		if node.Kind == yaml.MappingNode {
			for i := 0; i+1 < len(node.Content); i += 2 {
				key, value := node.Content[i], node.Content[i+1]
				if opaque && key.Value == "$ref" {
					candidate := fmt.Sprintf("x-kaji-opaque-reference-%d", len(keys))
					for {
						exists := false
						for j := 0; j+1 < len(node.Content); j += 2 {
							if node.Content[j].Value == candidate {
								exists = true
								break
							}
						}
						if !exists {
							break
						}
						candidate += "_"
					}
					key.Value = candidate
					keys = append(keys, key)
				}
				walk(value, opaque || !entries && strings.HasPrefix(key.Value, "x-"), key.Value == "schemas" || key.Value == "properties" || key.Value == "patternProperties" || key.Value == "$defs" || key.Value == "definitions")
			}
		} else {
			for _, child := range node.Content {
				walk(child, opaque, false)
			}
		}
	}
	walk(root, false, false)
	return func() {
		for _, key := range keys {
			key.Value = "$ref"
		}
	}
}
