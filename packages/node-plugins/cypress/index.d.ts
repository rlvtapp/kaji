import type { NativeAddon } from '@relevate/kaji/sdk';

export interface CypressPluginOptions { target?: string; output?: string }
export function pluginCypress(options?: CypressPluginOptions): NativeAddon;
