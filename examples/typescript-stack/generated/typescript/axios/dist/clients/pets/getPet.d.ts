import type { Options, Unwrappable, RequestResult } from '../../.poolster/client';
import type { GetPetOptions, GetPetResponses } from '../../models/pets/GetPet';
/**
 * {@link /pets/:petId}
 */
export declare function getPet<ThrowOnError extends boolean = true>(options: Options<GetPetOptions, ThrowOnError>): Unwrappable<RequestResult<GetPetResponses, ThrowOnError>>;
