import type { Options, Unwrappable, RequestResult } from '../../.kaji/client';
import type { CreatePetOptions, CreatePetResponses } from '../../models/pets/CreatePet';
/**
 * {@link /pets}
 */
export declare function createPet<ThrowOnError extends boolean = true>(options: Options<CreatePetOptions, ThrowOnError>): Unwrappable<RequestResult<CreatePetResponses, ThrowOnError>>;
