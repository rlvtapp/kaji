import { createClient } from './.kaji/client';
import { PetsClient } from './resources/pets';
export class Pets {
    /** Configured request transport for direct operations and generated framework hooks. */
    transport;
    pets;
    constructor(config = {}) {
        this.transport = createClient(config);
        this.pets = new PetsClient(this.transport);
    }
}
