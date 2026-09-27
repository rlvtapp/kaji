/* eslint-disable no-alert, no-console */
import { client, withUnwrap } from '../../.kaji/client';
/**
 * {@link /pets}
 */
export function createPet(options) {
    const { client: request = client, ...config } = options;
    return withUnwrap(request({ method: 'POST', url: '/pets', ...config, throwOnError: config.throwOnError ?? true }));
}
