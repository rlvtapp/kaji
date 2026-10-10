//! Manifest implementation for generated typescript-cli packages.

pub(super) fn render_package_json(package_name: &str, command_name: &str, version: &str) -> String {
    format!(
        "{{\n  \"name\": {package_name:?},\n  \"version\": {version:?},\n  \"private\": true,\n  \"description\": \"Generated API CLI\",\n  \"type\": \"module\",\n  \"bin\": {{ {command_name:?}: \"dist/index.js\" }},\n  \"files\": [\"dist\", \"references\", \"README.md\"],\n  \"scripts\": {{ \"build\": \"tsc\", \"start\": \"node dist/index.js\" }},\n  \"engines\": {{ \"node\": \">=20\" }},\n  \"dependencies\": {{ \"commander\": \"^13.0.0\" }},\n  \"devDependencies\": {{ \"@types/node\": \"^22.0.0\", \"typescript\": \"^5.7.0\" }}\n}}\n"
    )
}

pub(super) fn render_tsconfig() -> &'static str {
    "{\n  \"compilerOptions\": {\n    \"target\": \"ES2022\",\n    \"module\": \"NodeNext\",\n    \"moduleResolution\": \"NodeNext\",\n    \"outDir\": \"dist\",\n    \"rootDir\": \"src\",\n    \"strict\": true,\n    \"esModuleInterop\": true,\n    \"skipLibCheck\": true\n  },\n  \"include\": [\"src/**/*.ts\"]\n}\n"
}

pub(super) fn render_extension() -> &'static str {
    "// This file is created once and never overwritten by Poolster.\n// Add provider-specific authentication, login, logging, tracing, or request customization here.\nimport type { CliExtension } from './runtime.js';\n\nexport const extension: CliExtension = {\n  // async authenticate({ headers }) { headers.set('authorization', `Bearer ${await readFromKeychain()}`); return 'handled'; },\n  // async login({ defaultLogin }) { return defaultLogin(); },\n  // async beforeRequest({ operation, url }) { console.debug(operation.id, url.toString()); },\n  // async afterResponse({ operation, response }) { console.info(operation.id, response.status); },\n};\n"
}
