# Add output plugins

← [JavaScript](README.md)

Choose a client and the helpers you need. Each helper targets a package path.

```js
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';
import { pluginReactQuery } from '@relevate/poolster-plugin-react-query';
import { pluginZod } from '@relevate/poolster-plugin-zod';

plugins: [
  pluginTypeScript({ path: 'web' }),
  pluginReactQuery({ target: 'web' }),
  pluginZod({ target: 'web' }),
]
```

| Plugin | Produces |
| --- | --- |
| React Query / Vue Query | Query and mutation hooks |
| SWR | Query and mutation hooks |
| Zod | Runtime validation schemas |
| Faker | Synthetic fixtures |
| MSW | Mock request handlers |
| Cypress | Request and interception helpers |

All seven support the documented HTTP and GraphQL subsets. GraphQL helpers use
selected operation types; they do not inherit every HTTP plugin option.
[GraphQL helper usage](../reference/outputs/graphql-integrations.md) gives examples and limits.

Only install the integrations your application needs. Build the emitted package
with its declared dependencies and install framework peers in your application.

**Next:** [Support matrix](../plugin-support-matrix.md) ·
[Write a JavaScript plugin](../plugins/javascript/output.md)
