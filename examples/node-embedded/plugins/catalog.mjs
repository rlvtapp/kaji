import { definePlugin } from '../../../packages/cli/sdk/index.mjs';

export const pluginCatalog = definePlugin(({ directory = 'catalog' } = {}) => ({
  name: 'catalog',
  hooks: {
    generate(ctx) {
      ctx.emitFile({
        path: `${directory}/README.md`,
        contents: `# ${ctx.api.name}\n\n${ctx.api.operations.length} operations, ${ctx.api.schemas.length} schemas.\n`,
      });
    },
    operation(operation, ctx) {
      ctx.emitFile({
        path: `${directory}/${operation.id}.md`,
        contents: `# ${operation.id}\n\n\`${operation.method} ${operation.path}\`\n`,
      });
    },
  },
}));
