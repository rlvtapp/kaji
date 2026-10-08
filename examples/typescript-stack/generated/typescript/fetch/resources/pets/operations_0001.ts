import type { ClientInstance } from '../../.poolster/client'
import { listPets } from '../../clients/pets/listPets'
import { createPet } from '../../clients/pets/createPet'
import { getPet } from '../../clients/pets/getPet'

export class PetsClientOperations0001 {
  readonly list: typeof listPets
  readonly create: typeof createPet
  readonly get: typeof getPet

  constructor(client: ClientInstance) {
    this.list = ((options: Parameters<typeof listPets>[0]) => listPets({ ...options, client })) as typeof listPets
    this.create = ((options: Parameters<typeof createPet>[0]) => createPet({ ...options, client })) as typeof createPet
    this.get = ((options: Parameters<typeof getPet>[0]) => getPet({ ...options, client })) as typeof getPet
  }
}
