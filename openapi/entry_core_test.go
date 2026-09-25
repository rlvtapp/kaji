package main

import (
	"flag"
	"os"
	"path/filepath"
	"testing"
)

func TestExecuteHelpers(t *testing.T) {
	dir := t.TempDir()
	specPath := filepath.Join(dir, "spec.yaml")
	if err := os.WriteFile(specPath, []byte(sampleSpec), 0o644); err != nil {
		t.Fatalf("write spec: %v", err)
	}
	outDir := filepath.Join(dir, "out")

	if err := execute(specPath, outDir); err != nil {
		t.Fatalf("execute: %v", err)
	}

	var prev *uint64
	hash, changed, count, err := executeWithHash(specPath, outDir, prev)
	if err != nil {
		t.Fatalf("executeWithHash: %v", err)
	}
	if !changed || count == 0 {
		t.Fatalf("expected changed with operations")
	}

	prev = &hash
	_, changed, _, err = executeWithHash(specPath, outDir, prev)
	if err != nil {
		t.Fatalf("executeWithHash cached: %v", err)
	}
	if changed {
		t.Fatalf("expected cached run to be unchanged")
	}
}

func TestMainWithArgs(t *testing.T) {
	oldArgs := os.Args
	oldFlag := flag.CommandLine
	defer func() {
		os.Args = oldArgs
		flag.CommandLine = oldFlag
	}()

	dir := t.TempDir()
	specPath := filepath.Join(dir, "spec.yaml")
	if err := os.WriteFile(specPath, []byte(sampleSpec), 0o644); err != nil {
		t.Fatalf("write spec: %v", err)
	}
	outDir := filepath.Join(dir, "out")

	flag.CommandLine = flag.NewFlagSet(os.Args[0], flag.ExitOnError)
	os.Args = []string{"relevate-openapi", "--out", outDir, specPath}

	main()
	if _, err := os.Stat(filepath.Join(outDir, "operations.json")); err != nil {
		t.Fatalf("expected operations.json: %v", err)
	}
}
