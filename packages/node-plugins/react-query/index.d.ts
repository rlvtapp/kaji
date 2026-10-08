import type { NativeAddon } from '@relevate/poolster';

export interface ReactQueryPluginOptions { target?: string; output?: string }
export function pluginReactQuery(options?: ReactQueryPluginOptions): NativeAddon;
