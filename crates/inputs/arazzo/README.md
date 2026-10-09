# poolster-input-arazzo

Native `arazzo` input provider. Register `ArazzoInput` directly with
`poolster_core::input::InputRegistry`, or enable `arazzo` in `poolster-inputs`.
The provider publishes `ArazzoDocument` for typed consumers in Poolster's plugin graph.

Executable workflows additionally publish
`poolster_core::native::workflows::WorkflowOperations` when callers provide
`InputOptions.workflow_sources`. Each key must match a declared source name;
values are explicit local OpenAPI files. Relative mappings resolve beside the
Arazzo document. Loading never downloads declared URLs or executes steps.

The first runner slice accepts Arazzo 1.0.0, 1.0.1 and 1.1.0 with local OpenAPI
3.0/3.1 sources. It supports sequential operation steps, local workflow
`dependsOn`, primitive path/query/header parameters, JSON request bodies,
primitive/object/array input types with presence/defaults, and one exact
`$statusCode == <integer>` success criterion per step. Without a criterion the
runner requires an HTTP 2xx response. Responses must contain JSON or be empty.

Supported value expressions are `$inputs.<name>`,
`$steps.<stepId>.outputs.<name>`, `$workflows.<dependencyId>.outputs.<name>`,
`$response.body#<JSON pointer>` and `$statusCode`; response expressions apply
only to step outputs. Objects and arrays recursively contain these expressions.
Bare operation IDs must resolve uniquely across sources. Qualified IDs use
`$sourceDescriptions.<name>.<operationId>`; operation paths use
`{$sourceDescriptions.<name>.url}#/paths/<escaped-path>/<method>`.

The runner rejects unresolved/ambiguous operations, nested workflow calls,
external dependencies, reusable parameters/actions, success/failure actions,
retries, JSONPath criteria, interpolation, unsupported input constraints,
non-JSON bodies, parameter serialization overrides, and authenticated OpenAPI
operations. GraphQL/Arazzo source descriptions are outside this executable
slice. Full upstream documents can remain inspectable while being unsupported
for execution; the official OAuth example is covered by this distinction.

The TypeScript `workflow` output publishes `WorkflowClient` symbols. Generated
functions accept typed workflow inputs; outputs retain `unknown` values rather
than claiming schema-derived output types. `runWorkflow(id, inputs, options)`
is also available. Dependency inputs default to the supplied root inputs and
may be overridden per workflow with `options.workflowInputs`. Endpoint overrides
are explicit `sourceBaseUrls`, and callers may provide `headers`, `fetch` and
an abort `signal`. Failures throw `WorkflowExecutionError` with completed steps,
dependency executions and the current response where available. Redirects are
rejected; workflows stop at the first failure and do not retry side effects.

`tests/workflows.rs` checks lowering and rejected features. The TypeScript
`workflow_native` integration suite compiles generated packages and executes
both a multi-step checkout and an authorization operation resolved from the
checksum-pinned official OAuth OpenAPI document against local HTTP servers.

Building blocks are optional. `WorkflowOperations::step_blocks(document_id)`
explicitly extracts `Blocks<WorkflowStepBlock>` for independent typed consumers;
the provider also publishes this collection for resolved inputs. Stable IDs use the
caller-selected document identity plus escaped workflow/step names, and metadata
keeps native JSON-pointer locations and references to earlier steps/dependency
workflows. Each value retains containing workflow inputs, dependency IDs and its
step position. A step block alone does not preserve workflow execution order or
control flow; executable runners must consume the whole workflow contract.
Custom transformers can re-extract blocks from their selected contract revision; custom consumers can
select it through the ordinary typed dependency graph.

Resolved inputs publish `Blocks<WorkflowStepBlock>` alongside `WorkflowOperations`, using the canonical document path as source identity and workflow/step names as local IDs. Unresolved inspection inputs retain `ArazzoDocument` and publish an empty step collection marked `Unavailable` with diagnostics. An empty `Complete` collection has different semantics. Block consumers still need the whole workflow contract for dependency and sequencing semantics.

The public `contracts` module re-exports the parser-independent Poolster workflow contracts; `blocks` exports `WorkflowStepBlocks`, `WorkflowStepBlock`, and `step_blocks`. Definitions remain in core to avoid input/output dependency cycles. Parser models remain in this input package.
