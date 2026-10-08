import type { NativeAddon } from '@relevate/kaji/sdk';

export interface FakerPluginOptions { target?: string; output?: string }
export function pluginFaker(options?: FakerPluginOptions): NativeAddon;
