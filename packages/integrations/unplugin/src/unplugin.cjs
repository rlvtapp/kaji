'use strict';

const poolster = require('./index.cjs');

function loadUnplugin() {
  try {
    return require('unplugin');
  } catch {
    throw new Error('The bundler adapter entry points require `unplugin`. Install it with your bundler, then import @relevate/unplugin-poolster/vite, /rollup, /webpack, /esbuild, /rspack, /rolldown, or /farm.');
  }
}

const { createUnplugin } = loadUnplugin();
module.exports = createUnplugin((options) => poolster(options));
