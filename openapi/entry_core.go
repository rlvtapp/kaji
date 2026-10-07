package main

import (
	"flag"
	"fmt"
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
	flag.Parse()

	if flag.NArg() < 1 {
		fmt.Println("Usage: kaji-openapi [--out <directory>] [--source-url <HTTP(S) URL>] <openapi-file>")
		os.Exit(1)
	}

	specPath := flag.Arg(0)

	if err := run(specPath, *outDir); err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
}
