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

func main() {
	outDir := flag.String("out", "latest", "output directory for generated files")
	flag.Parse()

	if flag.NArg() < 1 {
		fmt.Println("Usage: kaji-openapi [--out <directory>] <openapi-file>")
		os.Exit(1)
	}

	specPath := flag.Arg(0)

	if err := run(specPath, *outDir); err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
}
