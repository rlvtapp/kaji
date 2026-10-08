import type { NativeAddon } from '@relevate/poolster';

export interface CypressPluginOptions { target?: string; output?: string }
export function pluginCypress(options?: CypressPluginOptions): NativeAddon;
