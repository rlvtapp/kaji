package main

import (
	"errors"
	"strings"

	"github.com/pb33f/libopenapi/index"
	"github.com/pb33f/libopenapi/utils"
)

// The neutral contract preserves recursive schema references. Parser cycle
// diagnostics do not require eager expansion, even for shapes with no finite
// instance. Do not dismiss missing references or non-schema object cycles.
func onlySchemaCircularErrors(err error) bool {
	failures := utils.UnwrapErrors(err)
	if len(failures) == 0 {
		return false
	}
	for _, failure := range failures {
		var resolving *index.ResolvingError
		if !errors.As(failure, &resolving) || resolving.CircularReference == nil {
			return false
		}
		circle := resolving.CircularReference
		if len(circle.Journey) == 0 {
			return false
		}
		for _, reference := range circle.Journey {
			if reference == nil || !strings.HasPrefix(reference.Definition, "#/components/schemas/") {
				return false
			}
		}
	}
	return true
}
