import type { NativeAddon } from '@relevate/poolster';

export interface FakerPluginOptions { target?: string; output?: string }
export function pluginFaker(options?: FakerPluginOptions): NativeAddon;
