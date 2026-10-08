/* eslint-disable no-alert, no-console */
import { client, withUnwrap } from '../../.poolster/client';
/**
 * {@link /pets}
 */
export function listPets(options) {
    const { client: request = client, ...config } = options;
    return withUnwrap(request({ method: 'GET', url: '/pets', ...config, throwOnError: config.throwOnError ?? true }));
}
