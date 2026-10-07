package main

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/cespare/xxhash/v2"
	yaml "go.yaml.in/yaml/v4"
)

type SourceFileDoc struct {
	Path   string `json:"path"`
	SHA256 string `json:"sha256"`
}
type SourceClosureDoc struct {
	SHA256 string          `json:"sha256"`
	Files  []SourceFileDoc `json:"files"`
}
type sourceClosure struct {
	Documents map[string][]byte
	Manifest  SourceClosureDoc
	Hash      uint64
	Files     []string
	Base      string
	Root      string
}

// Hash the entire bounded reference closure before considering a cache hit.
// Remote documents use public HTTPS only and never inherit source credentials.
func collectSourceClosure(specPath string, root []byte) (*sourceClosure, error) {
	return collectSourceClosureWithOrigin(specPath, root, sourceDocumentURL, fetchRemoteSource)
}

func collectSourceClosureWithFetcher(specPath string, root []byte, fetch func(string) ([]byte, error)) (*sourceClosure, error) {
	return collectSourceClosureWithOrigin(specPath, root, "", fetch)
}

func collectSourceClosureWithOrigin(specPath string, root []byte, origin string, fetch func(string) ([]byte, error)) (*sourceClosure, error) {
	absolute, err := filepath.Abs(specPath)
	if err != nil {
		return nil, err
	}
	base := filepath.Dir(absolute)
	rootID := absolute
	if origin != "" {
		uri, err := url.Parse(origin)
		if err != nil || (uri.Scheme != "https" && uri.Scheme != "http") || uri.Host == "" || uri.User != nil {
			return nil, fmt.Errorf("source-url must be an HTTP(S) document URL without credentials")
		}
		uri.Fragment = ""
		rootID = uri.String()
	}

	documents := map[string][]byte{}
	totalBytes := 0
	var visit func(string, []byte) error
	visit = func(path string, data []byte) error {
		if !isRemoteDocument(path) {
			path = filepath.Clean(path)
		}
		if _, exists := documents[path]; exists {
			return nil
		}
		if len(documents) >= 16384 {
			return fmt.Errorf("local reference closure exceeds 16384 documents")
		}
		if len(data) > 16<<20 {
			return fmt.Errorf("local reference document %s exceeds 16 MiB", path)
		}
		totalBytes += len(data)
		if totalBytes > 256<<20 {
			return fmt.Errorf("local reference closure exceeds 256 MiB")
		}
		documents[path] = data
		var node yaml.Node
		if err := yaml.Unmarshal(data, &node); err != nil {
			return fmt.Errorf("parse local reference document %s: %w", path, err)
		}
		follow := func(value string) error {
			target, _, err := resolveSourceReference(path, value)
			if err != nil {
				return fmt.Errorf("invalid reference in %s: %w", path, err)
			}
			if _, exists := documents[target]; exists {
				return nil
			}
			if isRemoteDocument(target) {
				bytes, err := fetch(target)
				if err != nil {
					return fmt.Errorf("fetch remote reference %s: %w", target, err)
				}
				return visit(target, bytes)
			}
			info, err := os.Stat(target)
			if err != nil {
				return fmt.Errorf("read local reference %q from %s: %w", value, path, err)
			}
			if info.Size() > 16<<20 {
				return fmt.Errorf("local reference document %s exceeds 16 MiB", target)
			}
			bytes, err := os.ReadFile(target)
			if err != nil {
				return fmt.Errorf("read local reference %q from %s: %w", value, path, err)
			}
			return visit(target, bytes)
		}
		var walk func(*yaml.Node, bool, bool) error
		walk = func(n *yaml.Node, isSchema, schemaEntries bool) error {
			if n.Kind == yaml.MappingNode {
				if !schemaEntries && (mappingValue(n, "type") != nil || mappingValue(n, "properties") != nil || mappingValue(n, "allOf") != nil || mappingValue(n, "oneOf") != nil || mappingValue(n, "anyOf") != nil) {
					isSchema = true
				}
				if property := mappingValue(n, "propertyName"); isSchema && property != nil && property.Kind == yaml.ScalarNode {
					if mapping := mappingValue(n, "mapping"); mapping != nil {
						for i := 1; i < len(mapping.Content); i += 2 {
							value := mapping.Content[i]
							if value.Kind == yaml.ScalarNode && strings.ContainsAny(value.Value, "/#.") {
								if err := follow(value.Value); err != nil {
									return err
								}
							}
						}
					}
				}
				for i := 0; i+1 < len(n.Content); i += 2 {
					key, value := n.Content[i], n.Content[i+1]
					if key.Value == "$ref" && value.Kind == yaml.ScalarNode {
						if err := follow(value.Value); err != nil {
							return err
						}
					}
					if isSchema && !schemaEntries && (key.Value == "default" || key.Value == "const" || key.Value == "enum" || key.Value == "example" || key.Value == "examples") {
						continue
					}
					nextSchema := isSchema || schemaEntries || key.Value == "schema" || key.Value == "itemSchema"
					entries := key.Value == "schemas" && !isSchema || isSchema && (key.Value == "properties" || key.Value == "patternProperties" || key.Value == "$defs" || key.Value == "definitions")
					if err := walk(value, nextSchema, entries); err != nil {
						return err
					}
				}
			} else {
				for _, child := range n.Content {
					if err := walk(child, isSchema, false); err != nil {
						return err
					}
				}
			}
			return nil
		}
		return walk(&node, false, false)
	}
	if err := visit(rootID, root); err != nil {
		return nil, err
	}
	files := make([]string, 0, len(documents))
	var total int
	for path, bytes := range documents {
		files = append(files, path)
		total += len(bytes)
	}
	if total > 256<<20 {
		return nil, fmt.Errorf("local reference closure exceeds 256 MiB")
	}
	sort.Slice(files, func(i, j int) bool { return files[i] < files[j] })
	source := &sourceClosure{Base: base, Root: rootID, Files: files, Documents: documents}
	hash := sha256.New()
	incremental := xxhash.New()
	for _, path := range files {
		relative := path
		if !isRemoteDocument(path) {
			relative, err = filepath.Rel(base, path)
			if err != nil {
				return nil, err
			}
		}
		relative = filepath.ToSlash(relative)
		bytes := documents[path]
		digest := sha256.Sum256(bytes)
		source.Manifest.Files = append(source.Manifest.Files, SourceFileDoc{Path: relative, SHA256: hex.EncodeToString(digest[:])})
		for _, writer := range []interface{ Write([]byte) (int, error) }{hash, incremental} {
			var length [8]byte
			binary.BigEndian.PutUint64(length[:], uint64(len(relative)))
			writer.Write(length[:])
			writer.Write([]byte(relative))
			binary.BigEndian.PutUint64(length[:], uint64(len(bytes)))
			writer.Write(length[:])
			writer.Write(bytes)
		}
	}
	source.Manifest.SHA256 = hex.EncodeToString(hash.Sum(nil))
	source.Hash = incremental.Sum64()
	if len(files) == 1 {
		digest := sha256.Sum256(root)
		source.Manifest.SHA256 = hex.EncodeToString(digest[:])
		source.Hash = computeSpecHash(root)
	}
	return source, nil
}
