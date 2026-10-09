import type { NativeAddon, FixtureOptions, CypressOptions } from '@relevate/poolster';

export interface FakerPluginOptions { target?: string; output?: string; fixtureOptions?: FixtureOptions }
export function pluginFaker(options?: FakerPluginOptions): NativeAddon;
