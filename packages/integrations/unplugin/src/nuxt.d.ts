import poolster = require('./index');

declare namespace poolsterNuxtModule {
  export interface PoolsterNuxtModule {
    meta: {
      name: string;
      configKey: string;
    };
    setup(options: poolster.PoolsterOptions): void;
  }
}

declare const poolsterNuxtModule: poolsterNuxtModule.PoolsterNuxtModule;
export = poolsterNuxtModule;
