package main

import (
	"fmt"
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
			if mt.ItemSchema != nil {
				return fmt.Errorf("%s content %s: unsupported OpenAPI itemSchema streaming representation; use supported explicit streaming plugins with schema until itemSchema codecs are implemented", where, p.Key())
			}
			if low := mt.GoLow(); low != nil && low.RootNode != nil {
				node := low.RootNode
				for i := 0; i+1 < len(node.Content); i += 2 {
					key := node.Content[i].Value
					if key == "prefixEncoding" || key == "itemEncoding" {
						return fmt.Errorf("%s content %s: unsupported OpenAPI %s positional/streaming multipart encoding", where, p.Key(), key)
					}
				}
			}
		}
		return nil
	}
	checkParams := func(params []*v3.Parameter, where string) error {
		for _, p := range params {
			if p != nil && p.In == "querystring" {
				return fmt.Errorf("%s: unsupported OpenAPI querystring parameter %q; whole-query content serialization is not equivalent to named query parameters", where, p.Name)
			}
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
			if low := item.GoLow(); low != nil && low.RootNode != nil {
				node := low.RootNode
				for i := 0; i+1 < len(node.Content); i += 2 {
					if node.Content[i].Value == "additionalOperations" && len(node.Content[i+1].Content) > 0 {
						return fmt.Errorf("%s: unsupported OpenAPI additionalOperations custom HTTP methods; Kaji's typed method IR supports GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS/TRACE/QUERY", where)
					}
				}
			}
			if err := checkParams(item.Parameters, where); err != nil {
				return err
			}
			if item.AdditionalOperations != nil && item.AdditionalOperations.Len() > 0 {
				return fmt.Errorf("%s: unsupported OpenAPI additionalOperations custom HTTP methods; Kaji's typed method IR supports GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS/TRACE/QUERY", where)
			}
			ops := item.GetOperations()
			if ops == nil {
				continue
			}
			for op := ops.First(); op != nil; op = op.Next() {
				value := op.Value()
				if value == nil {
					continue
				}
				label := where + " " + strings.ToUpper(op.Key())
				if err := checkParams(value.Parameters, label); err != nil {
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
