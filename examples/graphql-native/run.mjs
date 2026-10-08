import { createGraphqlServer } from './server.mjs';
import { runConsumer } from './consumer.mjs';
const server = createGraphqlServer();
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
try {
  await runConsumer(`http://127.0.0.1:${server.address().port}`);
} finally {
  await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
}
