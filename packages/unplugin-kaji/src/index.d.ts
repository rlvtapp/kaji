declare namespace poolster {
  export interface PoolsterResult {
    command: string;
    args: string[];
    stdout: string;
    stderr: string;
    code: number | null;
    signal: string | null;
  }

  export interface PoolsterOptions {
    /** Poolster recipe, relative to cwd. Defaults to `poolster.json`. Set false with args. */
    config?: string | false;
    /** Exact Poolster CLI arguments, for direct generation instead of a recipe. */
    args?: string[];
    /** Working directory used for generation and relative watch paths. */
    cwd?: string;
    /** A Poolster executable. By default the installed poolster launcher is used. */
    command?: string;
    /** Path to a Node launcher; mainly useful for local development and tests. */
    launcher?: string;
    env?: Record<string, string | undefined>;
    /** Additional local files which trigger generation in watch mode. */
    watchFiles?: string[];
    /** Regenerate for watched file changes. Defaults to true. */
    watch?: boolean;
    /** Do not forward Poolster output to the terminal. */
    silent?: boolean;
    onGenerate?: (result: PoolsterResult) => void;
  }

  export interface PoolsterPlugin {
    name: string;
    enforce: 'pre';
    buildStart(this: { addWatchFile?: (id: string) => void }): Promise<PoolsterResult>;
    watchChange(this: { addWatchFile?: (id: string) => void }, id: string): Promise<PoolsterResult> | undefined;
  }
}

declare function poolster(options?: poolster.PoolsterOptions): poolster.PoolsterPlugin;
export = poolster;
