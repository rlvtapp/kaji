# GitHub GraphQL corpus

This is the unmodified GitHub API schema maintained by Octokit, pinned to commit
`503d8f18a265230ec224669241c2f1957811faf8`. It is 1,177,658 bytes. The adjacent
`provenance.json` records immutable source URLs, SHA-256 digests and sizes;
`LICENSE.md` retains the upstream MIT license. These fixtures run offline in the
normal provider test suite.

The full-schema tests validate its native schema, check representative types and
query/mutation roots, verify provider publication, and deliberately break a type
reference to ensure a large document cannot bypass semantic validation.

An initially evaluated newer upstream snapshot,
`82ff2d4780080e6929ebb95608cefa22dfa05ac7`, failed strict semantic validation:
`EnterpriseOwnerInfo` repeats `repositoryDeployKeySetting` and
`repositoryDeployKeySettingOrganizations`. It was rejected rather than patched or
accepted with reduced validation. The historical valid snapshot above makes the
positive regression test reproducible.
