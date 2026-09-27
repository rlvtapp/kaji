import { PetsClientOperations0001 } from './pets/operations_0001';
export class PetsClient {
    constructor(client) {
        Object.assign(this, new PetsClientOperations0001(client));
    }
}
