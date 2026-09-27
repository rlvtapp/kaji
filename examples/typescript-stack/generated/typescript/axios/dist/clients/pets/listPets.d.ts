import type { Options, Unwrappable, RequestResult } from '../../.kaji/client';
import type { ListPetsOptions, ListPetsResponses } from '../../models/pets/ListPets';
/**
 * {@link /pets}
 */
export declare function listPets<ThrowOnError extends boolean = true>(options: Options<ListPetsOptions, ThrowOnError>): Unwrappable<RequestResult<ListPetsResponses, ThrowOnError>>;
