import kaji = require('./index');

declare namespace astroKaji {
  export interface AstroKajiIntegration {
    name: string;
    hooks: {
      'astro:config:setup': (astro: {
        config: { vite?: { plugins?: unknown[] } };
      }) => void;
    };
  }
}

declare function astroKaji(options?: kaji.KajiOptions): astroKaji.AstroKajiIntegration;
export = astroKaji;
