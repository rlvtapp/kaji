import type { NativeAddon } from '@relevate/kaji/sdk';

export interface ReactQueryPluginOptions { target?: string; output?: string }
export function pluginReactQuery(options?: ReactQueryPluginOptions): NativeAddon;
