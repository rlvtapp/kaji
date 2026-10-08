import type { ClientInstance } from '../.poolster/client';
import { PetsClientOperations0001 } from './pets/operations_0001';
export interface PetsClient extends PetsClientOperations0001 {
}
export declare class PetsClient {
    constructor(client: ClientInstance);
}
