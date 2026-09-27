/* eslint-disable no-alert, no-console */

import type { Options, Unwrappable, RequestResult } from '../../.kaji/client'
import type { CreatePetOptions, CreatePetResponses } from '../../models/pets/CreatePet'
import { client, withUnwrap } from '../../.kaji/client'

/**
 * {@link /pets}
 */
export function createPet<ThrowOnError extends boolean = true>(
  options: Options<CreatePetOptions, ThrowOnError>,
): Unwrappable<RequestResult<CreatePetResponses, ThrowOnError>> {
  const { client: request = client, ...config } = options

  return withUnwrap(
    request({ method: 'POST', url: '/pets', ...config, throwOnError: config.throwOnError ?? true }) as Promise<RequestResult<CreatePetResponses, ThrowOnError>>,
  )
}
