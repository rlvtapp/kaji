import type { ClientConfig, ClientInstance } from './.kaji/client';
import { PetsClient } from './resources/pets';
export declare class Pets {
    /** Configured request transport for direct operations and generated framework hooks. */
    readonly transport: ClientInstance;
    readonly pets: PetsClient;
    constructor(config?: ClientConfig);
}
