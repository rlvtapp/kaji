import type { NativeAddon, FixtureOptions, CypressOptions } from '@relevate/poolster';

export interface MswPluginOptions { target?: string; output?: string; fixtureOptions?: FixtureOptions }
export function pluginMsw(options?: MswPluginOptions): NativeAddon;
