import type { ClientConfig, ClientInstance } from './.poolster/client'
import { createClient } from './.poolster/client'
import { PetsClient } from './resources/pets'

export class Pets {
  /** Configured request transport for direct operations and generated framework hooks. */
  readonly transport: ClientInstance
  readonly pets: PetsClient

  constructor(config: ClientConfig = {}) {
    this.transport = createClient(config)
    this.pets = new PetsClient(this.transport)
  }
}
