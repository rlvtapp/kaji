import poolster = require('./index');

declare namespace astroPoolster {
  export interface AstroPoolsterIntegration {
    name: string;
    hooks: {
      'astro:config:setup': (astro: {
        config: { vite?: { plugins?: unknown[] } };
      }) => void;
    };
  }
}

declare function astroPoolster(options?: poolster.PoolsterOptions): astroPoolster.AstroPoolsterIntegration;
export = astroPoolster;
