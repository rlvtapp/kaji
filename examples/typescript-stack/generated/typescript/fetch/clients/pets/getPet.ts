/* eslint-disable no-alert, no-console */

import type { Options, Unwrappable, RequestResult } from '../../.kaji/client'
import type { GetPetOptions, GetPetResponses } from '../../models/pets/GetPet'
import { client, withUnwrap } from '../../.kaji/client'

/**
 * {@link /pets/:petId}
 */
export function getPet<ThrowOnError extends boolean = true>(
  options: Options<GetPetOptions, ThrowOnError>,
): Unwrappable<RequestResult<GetPetResponses, ThrowOnError>> {
  const { client: request = client, ...config } = options

  return withUnwrap(
    request({ method: 'GET', url: '/pets/{petId}', ...config, throwOnError: config.throwOnError ?? true }) as Promise<
      RequestResult<GetPetResponses, ThrowOnError>
    >,
  )
}
