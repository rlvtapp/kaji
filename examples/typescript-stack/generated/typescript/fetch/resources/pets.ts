import type { ClientInstance } from '../.kaji/client'
import { PetsClientOperations0001 } from './pets/operations_0001'

export interface PetsClient extends PetsClientOperations0001 {}

export class PetsClient {
  constructor(client: ClientInstance) {
    Object.assign(this, new PetsClientOperations0001(client))
  }
}
