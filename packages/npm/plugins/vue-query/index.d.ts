import type { NativeAddon, FixtureOptions, CypressOptions } from '@relevate/poolster';

export interface VueQueryPluginOptions { target?: string; output?: string }
export function pluginVueQuery(options?: VueQueryPluginOptions): NativeAddon;
