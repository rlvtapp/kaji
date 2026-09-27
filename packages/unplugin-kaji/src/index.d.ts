declare namespace kaji {
  export interface KajiResult {
    command: string;
    args: string[];
    stdout: string;
    stderr: string;
    code: number | null;
    signal: string | null;
  }

  export interface KajiOptions {
    /** Kaji recipe, relative to cwd. Defaults to `kaji.json`. Set false with args. */
    config?: string | false;
    /** Exact Kaji CLI arguments, for direct generation instead of a recipe. */
    args?: string[];
    /** Working directory used for generation and relative watch paths. */
    cwd?: string;
    /** A Kaji executable. By default the installed @relevate/kaji launcher is used. */
    command?: string;
    /** Path to a Node launcher; mainly useful for local development and tests. */
    launcher?: string;
    env?: Record<string, string | undefined>;
    /** Additional local files which trigger generation in watch mode. */
    watchFiles?: string[];
    /** Regenerate for watched file changes. Defaults to true. */
    watch?: boolean;
    /** Do not forward Kaji output to the terminal. */
    silent?: boolean;
    onGenerate?: (result: KajiResult) => void;
  }

  export interface KajiPlugin {
    name: string;
    enforce: 'pre';
    buildStart(this: { addWatchFile?: (id: string) => void }): Promise<KajiResult>;
    watchChange(this: { addWatchFile?: (id: string) => void }, id: string): Promise<KajiResult> | undefined;
  }
}

declare function kaji(options?: kaji.KajiOptions): kaji.KajiPlugin;
export = kaji;
