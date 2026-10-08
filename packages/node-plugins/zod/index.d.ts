import type { NativeAddon } from '@relevate/kaji/sdk';

export interface ZodPluginOptions { target?: string; output?: string }
export function pluginZod(options?: ZodPluginOptions): NativeAddon;
