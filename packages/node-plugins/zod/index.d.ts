import type { NativeAddon } from '@relevate/poolster';

export interface ZodPluginOptions { target?: string; output?: string }
export function pluginZod(options?: ZodPluginOptions): NativeAddon;
