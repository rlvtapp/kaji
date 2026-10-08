'use strict';

const { addVitePlugin, addWebpackPlugin, defineNuxtModule } = require('@nuxt/kit');
const vite = require('./vite.cjs');
const webpack = require('./webpack.cjs');

/**
 * Nuxt selects its Vite or webpack builder at runtime. Registering both
 * adapters lets Poolster run before whichever builder the application uses.
 */
module.exports = defineNuxtModule({
  meta: {
    name: '@relevate/unplugin-poolster',
    configKey: 'poolster',
  },
  setup(options) {
    addVitePlugin(() => vite(options));
    addWebpackPlugin(() => webpack(options));
  },
});
