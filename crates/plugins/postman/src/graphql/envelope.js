let response;
let valid = false;
try {
  response = pm.response.json();
  valid = response !== null && typeof response === 'object' && !Array.isArray(response)
    && (!Object.prototype.hasOwnProperty.call(response, 'data') || response.data === null || typeof response.data === 'object' && !Array.isArray(response.data))
    && (Object.prototype.hasOwnProperty.call(response, 'data') || Array.isArray(response.errors) && response.errors.length > 0)
    && (!Object.prototype.hasOwnProperty.call(response, 'errors') || Array.isArray(response.errors) && response.errors.length > 0 && response.errors.every(error => error !== null && typeof error === 'object' && typeof error.message === 'string'));
} catch (_) {}
pm.test('GraphQL response envelope is valid', function () { pm.expect(valid).to.equal(true); });
const errors = valid && response.errors || [];
const hasData = valid && Object.prototype.hasOwnProperty.call(response, 'data') && response.data !== null;
pm.collectionVariables.set('poolster_graphql_status', !valid ? 'protocol_error' : errors.length ? hasData ? 'partial' : 'error' : 'success');
pm.collectionVariables.set('poolster_graphql_errors', JSON.stringify(errors));
