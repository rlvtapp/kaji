import { listPets } from '../../clients/pets/listPets';
import { createPet } from '../../clients/pets/createPet';
import { getPet } from '../../clients/pets/getPet';
export class PetsClientOperations0001 {
    list;
    create;
    get;
    constructor(client) {
        this.list = ((options) => listPets({ ...options, client }));
        this.create = ((options) => createPet({ ...options, client }));
        this.get = ((options) => getPet({ ...options, client }));
    }
}
