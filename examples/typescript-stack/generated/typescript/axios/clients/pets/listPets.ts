/* eslint-disable no-alert, no-console */

import type { Options, Unwrappable, RequestResult } from '../../.kaji/client'
import type { ListPetsOptions, ListPetsResponses } from '../../models/pets/ListPets'
import { client, withUnwrap } from '../../.kaji/client'

/**
 * {@link /pets}
 */
export function listPets<ThrowOnError extends boolean = true>(
  options: Options<ListPetsOptions, ThrowOnError>,
): Unwrappable<RequestResult<ListPetsResponses, ThrowOnError>> {
  const { client: request = client, ...config } = options

  return withUnwrap(
    request({ method: 'GET', url: '/pets', ...config, throwOnError: config.throwOnError ?? true }) as Promise<RequestResult<ListPetsResponses, ThrowOnError>>,
  )
}
