'use strict';

const unplugin = require('./unplugin.cjs');

/**
 * An Astro integration that adds Poolster's Vite adapter to Astro's Vite config.
 * Poolster still runs through the normal build-start and watch hooks, keeping the
 * behaviour identical to a direct Vite configuration.
 */
module.exports = function astroPoolster(options) {
  return {
    name: '@relevate/unplugin-poolster',
    hooks: {
      'astro:config:setup': (astro) => {
        astro.config.vite ||= {};
        astro.config.vite.plugins ||= [];
        astro.config.vite.plugins.push(unplugin.vite(options));
      },
    },
  };
};
