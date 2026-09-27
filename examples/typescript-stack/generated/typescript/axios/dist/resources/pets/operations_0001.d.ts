import type { ClientInstance } from '../../.kaji/client';
import { listPets } from '../../clients/pets/listPets';
import { createPet } from '../../clients/pets/createPet';
import { getPet } from '../../clients/pets/getPet';
export declare class PetsClientOperations0001 {
    readonly list: typeof listPets;
    readonly create: typeof createPet;
    readonly get: typeof getPet;
    constructor(client: ClientInstance);
}
