import type { NativeAddon, FixtureOptions, CypressOptions } from '@relevate/poolster';

export interface ReactQueryPluginOptions { target?: string; output?: string }
export function pluginReactQuery(options?: ReactQueryPluginOptions): NativeAddon;
