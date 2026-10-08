package main

import (
	"encoding/json"
	"flag"
	"fmt"
	yaml "go.yaml.in/yaml/v4"
	"os"
)

func execute(path, outDir string) error {
	return run(path, outDir)
}

func executeWithHash(path, outDir string, prevHash *uint64) (uint64, bool, int, error) {
	return runWithHash(path, outDir, prevHash)
}

var sourceDocumentURL string

func main() {
	flag.StringVar(&sourceDocumentURL, "source-url", "", "original HTTP(S) URL for a downloaded root document (relative references only; no inherited authentication)")
	outDir := flag.String("out", "latest", "output directory for generated files")
	readDocument := flag.Bool("read-document", false, "read bounded YAML/JSON configuration as JSON")
	exportDocument := flag.Bool("export-openapi", false, "export an OpenAPI document with bundled references as JSON")
	flag.Parse()

	if flag.NArg() < 1 {
		fmt.Println("Usage: kaji-openapi [--out <directory>] [--source-url <HTTP(S) URL>] <openapi-file>")
		os.Exit(1)
	}

	specPath := flag.Arg(0)

	if *readDocument || *exportDocument {
		if err := exportJSONDocument(specPath, *exportDocument); err != nil {
			fmt.Fprintf(os.Stderr, "error: %v\n", err)
			os.Exit(1)
		}
		return
	}

	if err := run(specPath, *outDir); err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
}

func exportJSONDocument(path string, bundle bool) error {
	info, err := os.Stat(path)
	if err != nil {
		return err
	}
	limit := int64(2 * 1024 * 1024)
	if bundle {
		limit = 64 * 1024 * 1024
	}
	if info.Size() > limit {
		return fmt.Errorf("document exceeds %d bytes", limit)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	if bundle {
		if err := validateDocumentVersion(data); err != nil {
			return err
		}
		closure, err := collectSourceClosure(path, data)
		if err != nil {
			return err
		}
		if len(closure.Files) > 1 || len(closure.Identities) > 0 || closure.NeedsNormalization {
			data, err = bundleLocalSources(closure, path)
			if err != nil {
				return err
			}
		}
	}
	var node yaml.Node
	if err := unmarshalDocument(data, &node); err != nil {
		return err
	}
	value, err := yamlNodeToInterface(&node)
	if err != nil {
		return err
	}
	return json.NewEncoder(os.Stdout).Encode(value)
}
