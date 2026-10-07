package main

import (
	"errors"
	"github.com/pb33f/libopenapi/index"
	"testing"
)

func TestOnlySchemaCircularDiagnosticsAreAccepted(t *testing.T) {
	schema := &index.ResolvingError{CircularReference: &index.CircularReferenceResult{Journey: []*index.Reference{{Definition: "#/components/schemas/A"}, {Definition: "#/components/schemas/B"}}}}
	nonschema := &index.ResolvingError{CircularReference: &index.CircularReferenceResult{Journey: []*index.Reference{{Definition: "#/components/responses/A"}}}}
	if !onlySchemaCircularErrors(schema) {
		t.Fatal("schema cycle should preserve references")
	}
	for _, failure := range []error{nonschema, errors.New("missing ref"), errors.Join(schema, errors.New("missing ref")), &index.ResolvingError{}, errors.Join(schema, nonschema)} {
		if onlySchemaCircularErrors(failure) {
			t.Fatal("dismissed a non-schema or mixed error")
		}
	}
}
