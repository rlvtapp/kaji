import kaji = require('./index');

declare namespace kajiNuxtModule {
  export interface KajiNuxtModule {
    meta: {
      name: string;
      configKey: string;
    };
    setup(options: kaji.KajiOptions): void;
  }
}

declare const kajiNuxtModule: kajiNuxtModule.KajiNuxtModule;
export = kajiNuxtModule;
