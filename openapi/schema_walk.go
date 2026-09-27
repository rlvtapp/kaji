package main

import "github.com/pb33f/libopenapi/datamodel/high/base"

// Documentation projections are finite views, not schema dereferencing. Keep
// the original schema/$refs in the artifacts; stop only the preview traversal.
// A node budget also bounds wide, acyclic DAGs with many repeated references.
const schemaPreviewDepth = 32
const schemaPreviewNodes = 256

type schemaWalk struct {
	active    map[*base.Schema]bool
	depth     int
	remaining int
}

func newSchemaWalk() *schemaWalk {
	return &schemaWalk{active: make(map[*base.Schema]bool), remaining: schemaPreviewNodes}
}

func (walk *schemaWalk) enter(schema *base.Schema) bool {
	if schema == nil || walk.active[schema] || walk.depth >= schemaPreviewDepth || walk.remaining <= 0 {
		return false
	}
	walk.active[schema] = true
	walk.depth++
	walk.remaining--
	return true
}

func (walk *schemaWalk) leave(schema *base.Schema) {
	delete(walk.active, schema)
	walk.depth--
}
