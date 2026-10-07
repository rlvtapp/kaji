package main

import (
	"encoding/json"
	"fmt"
	v3 "github.com/pb33f/libopenapi/datamodel/high/v3"
	"go.yaml.in/yaml/v4"
)

func oauthDeviceURL(flow *v3.OAuthFlow) string {
	if flow.GoLow() == nil {
		return ""
	}
	node := mappingValue(flow.GoLow().RootNode, "deviceAuthorizationUrl")
	if node == nil {
		return ""
	}
	return node.Value
}

func convertMediaDefinition(contentType string, media *v3.MediaType) (MediaTypeDoc, error) {
	result := MediaTypeDoc{ContentType: contentType}
	var err error
	result.SchemaDefinition, err = schemaDefinitionFromMediaType(media)
	if err != nil {
		return result, err
	}
	result.Encoding, err = convertFormEncodings(media)
	if err != nil {
		return result, err
	}
	err = addMedia32(media, &result)
	return result, err
}

// The low source nodes preserve 3.2 positional encoding fields omitted or
// incorrectly typed by the high-level parser. Decode them into the same typed
// recursive transport representation used for named multipart properties.
func addMedia32(media *v3.MediaType, result *MediaTypeDoc) error {
	if media == nil {
		return nil
	}
	if media.ItemSchema != nil {
		value, err := schemaProxyToInterface(media.ItemSchema)
		if err != nil {
			return err
		}
		result.ItemSchemaDefinition = value
	}
	if media.GoLow() == nil {
		return nil
	}
	var err error
	result.PrefixEncoding, result.ItemEncoding, err = positionalEncodings(media.GoLow().RootNode)
	return err
}
func positionalEncodings(node *yaml.Node) ([]FormEncodingDoc, *FormEncodingDoc, error) {
	var prefix []FormEncodingDoc
	var item *FormEncodingDoc
	if values := mappingValue(node, "prefixEncoding"); values != nil {
		if values.Kind != yaml.SequenceNode {
			return nil, nil, fmt.Errorf("prefixEncoding must be an array")
		}
		for _, entry := range values.Content {
			encoding, err := decodeEncoding32(entry)
			if err != nil {
				return nil, nil, err
			}
			prefix = append(prefix, *encoding)
		}
	}
	if value := mappingValue(node, "itemEncoding"); value != nil {
		var err error
		item, err = decodeEncoding32(value)
		if err != nil {
			return nil, nil, err
		}
	}
	if named := mappingValue(node, "encoding"); named != nil && (mappingValue(node, "prefixEncoding") != nil || mappingValue(node, "itemEncoding") != nil) {
		return nil, nil, fmt.Errorf("encoding and positional prefixEncoding/itemEncoding are mutually exclusive")
	}
	return prefix, item, nil
}
func decodeEncoding32(node *yaml.Node) (*FormEncodingDoc, error) {
	if node.Kind != yaml.MappingNode {
		return nil, fmt.Errorf("Encoding Object must be an object")
	}
	value, err := yamlNodeToInterface(node)
	if err != nil {
		return nil, err
	}
	bytes, err := json.Marshal(value)
	if err != nil {
		return nil, err
	}
	var result FormEncodingDoc
	if err = json.Unmarshal(bytes, &result); err != nil {
		return nil, err
	}
	// Header schemas use the compiler IR name rather than the source field name.
	result.Headers = nil
	if headers := mappingValue(node, "headers"); headers != nil {
		result.Headers = map[string]FormHeaderDoc{}
		for i := 0; i+1 < len(headers.Content); i += 2 {
			h := headers.Content[i+1]
			raw, err := yamlNodeToInterface(h)
			if err != nil {
				return nil, err
			}
			b, _ := json.Marshal(raw)
			var header FormHeaderDoc
			if err = json.Unmarshal(b, &header); err != nil {
				return nil, err
			}
			header.SchemaDefinition, err = yamlNodeToInterface(mappingValue(h, "schema"))
			if err != nil {
				return nil, err
			}
			result.Headers[headers.Content[i].Value] = header
		}
	}
	result.PrefixEncoding, result.ItemEncoding, err = positionalEncodings(node)
	if err != nil {
		return nil, err
	}
	if named := mappingValue(node, "encoding"); named != nil {
		result.Encoding = map[string]FormEncodingDoc{}
		for i := 0; i+1 < len(named.Content); i += 2 {
			child, err := decodeEncoding32(named.Content[i+1])
			if err != nil {
				return nil, err
			}
			result.Encoding[named.Content[i].Value] = *child
		}
	}
	return &result, nil
}

func deviceAuthorizationFlow(node *yaml.Node) OAuthFlowDoc {
	result := OAuthFlowDoc{Type: "deviceAuthorization", Scopes: map[string]string{}}
	for name, target := range map[string]*string{"deviceAuthorizationUrl": &result.DeviceAuthorizationURL, "authorizationUrl": &result.AuthorizationURL, "tokenUrl": &result.TokenURL, "refreshUrl": &result.RefreshURL} {
		if value := mappingValue(node, name); value != nil {
			*target = value.Value
		}
	}
	if scopes := mappingValue(node, "scopes"); scopes != nil {
		for i := 0; i+1 < len(scopes.Content); i += 2 {
			result.Scopes[scopes.Content[i].Value] = scopes.Content[i+1].Value
		}
	}
	return result
}
