/* eslint-disable no-alert, no-console */
import { client, withUnwrap } from '../../.poolster/client';
/**
 * {@link /pets/:petId}
 */
export function getPet(options) {
    const { client: request = client, ...config } = options;
    return withUnwrap(request({ method: 'GET', url: '/pets/{petId}', ...config, throwOnError: config.throwOnError ?? true }));
}
