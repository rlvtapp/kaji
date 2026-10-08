import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, realpathSync } from 'node:fs';
import { isAbsolute, relative, resolve, sep } from 'node:path';
import { pathToFileURL } from 'node:url';

export function packageDirectory(workspace, input) {
  if (isAbsolute(input)) throw new Error('SDK path must be repository-relative');
  const root = realpathSync(workspace);
  const directory = realpathSync(resolve(root, input));
  const path = relative(root, directory);
  if (path === '..' || path.startsWith(`..${sep}`) || isAbsolute(path)) throw new Error('SDK path escapes repository');
  return directory;
}
const command = (program, ...args) => ({ program, args });
export function plan(directory, language) {
  const metadata = resolve(directory, '.poolster/package.json');
  if (existsSync(metadata)) {
    const value = JSON.parse(readFileSync(metadata, 'utf8'));
    if (value.schema_version !== 1 || value.language !== language) throw new Error('SDK metadata version/language mismatch');
    if (!Array.isArray(value.build) || !value.build.length || !Array.isArray(value.test) || !value.test.length) throw new Error('Plugin metadata must declare nonempty build and test commands');
    return [...value.build, ...value.test];
  }
  // Native defaults also work with SDKs generated before package metadata existed.
  switch (language) {
    case 'rust': return [command('cargo', 'test')];
    case 'typescript': return [command('npm', existsSync(resolve(directory, 'package-lock.json')) ? 'ci' : 'install', '--ignore-scripts'), command('npm', 'run', 'build')];
    case 'terraform': return [command('go', 'mod', 'tidy'), command('go', 'test', './...')];
    case 'go': return [command('go', 'test', './...')];
    case 'python': return [command('python', '-m', 'pip', 'install', '.'), command('python', '-m', 'compileall', '-q', 'src'), command('python', '-m', 'unittest', 'discover')];
    case 'java': return [command('mvn', '--batch-mode', '--no-transfer-progress', 'test')];
    case 'php': case 'symfony': return files(directory, '.php').map(file => command('php', '-l', file));
    case 'csharp': case 'dotnet': return [command('dotnet', 'build', '--configuration', 'Release'), command('dotnet', 'test', '--configuration', 'Release', '--no-build')];
    case 'elixir': return [command('mix', 'deps.get'), command('mix', 'compile', '--warnings-as-errors'), ...(existsSync(resolve(directory, 'test')) ? [command('mix', 'test')] : [])];
    case 'ruby': return files(directory, '.rb').map(file => command('ruby', '-c', file));
    case 'swift': return [command('swift', 'build'), ...(existsSync(resolve(directory, 'Tests')) ? [command('swift', 'test')] : [])];
    default: throw new Error(`Custom language ${language} requires plugin-declared build/test metadata and an installed toolchain`);
  }
}
function files(directory, extension) {
  const result = [];
  const visit = path => {
    for (const entry of readdirSync(path, { withFileTypes: true })) {
      if (['.git', 'vendor', 'node_modules', '.build'].includes(entry.name)) continue;
      const file = resolve(path, entry.name);
      if (entry.isDirectory()) visit(file);
      else if (entry.isFile() && file.endsWith(extension)) result.push(relative(directory, file));
    }
  };
  visit(directory);
  if (!result.length) throw new Error(`No ${extension} source files found`);
  return result.sort();
}
export function execute(commands, directory, run = spawnSync) {
  for (const { program, args = [] } of commands) {
    if (typeof program !== 'string' || !program || program.startsWith('-') || /[\0\r\n]/.test(program) || !Array.isArray(args) || args.some(arg => typeof arg !== 'string' || arg.includes('\0'))) throw new Error('Invalid package command');
    // Do not log arguments: a custom command can contain a credential.
    console.log(`Checking SDK with ${program}`);
    const result = run(program, args, { cwd: directory, stdio: 'inherit', shell: false });
    if (result.error || result.status !== 0) throw new Error(`${program} failed`);
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const directory = packageDirectory(process.env.GITHUB_WORKSPACE || process.cwd(), process.env.SDK_PATH || '.');
    execute(plan(directory, process.env.SDK_LANGUAGE), directory);
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
