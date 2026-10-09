import { requestReadUser, requestRename, interceptReadUser } from '../../sdk/dist/cypress.js';
import { createClient } from '../../sdk/dist/index.js';

describe('generated GraphQL Cypress helpers', () => {
  it('sends typed query and mutation documents to a local GraphQL server', () => {
    requestReadUser({ id: '42' }, { url: '/graphql' }).then(response => {
      expect(response.status).to.equal(200);
      expect(response.body.data.user).to.deep.equal({ id: '42', name: 'Ada', nickname: null });
    });
    requestRename({ name: 'Grace' }, { url: '/graphql' }).then(response => {
      expect(response.body.data.rename.name).to.equal('Grace');
    });
  });

  it('intercepts one operation and lets unrelated operations reach the server', () => {
    interceptReadUser({ data: { user: { id: '42', name: 'Mocked', nickname: null } } });
    cy.visit('/');
    cy.window().then(async win => {
      const client = createClient({ endpoint: `${win.location.origin}/graphql`, fetch: win.fetch.bind(win) });
      const renamed = await client.rename({ name: 'Real mutation' });
      expect(renamed.kind).to.equal('success');
      expect(renamed.data.rename.name).to.equal('Real mutation');
      const read = await client.readUser({ id: '42' });
      expect(read.kind).to.equal('success');
      expect(read.data.user.name).to.equal('Mocked');
    });
    cy.wait('@ReadUser').its('request.body.operationName').should('equal', 'ReadUser');
    cy.get('@ReadUser.all').should('have.length', 1);
  });

  it('retains partial GraphQL data and errors through an intercepted response', () => {
    interceptReadUser({
      data: { user: { id: '42', name: 'Partial', nickname: null } },
      errors: [{ message: 'nickname unavailable', path: ['user', 'nickname'] }],
    });
    cy.visit('/');
    cy.window().then(async win => {
      const client = createClient({ endpoint: `${win.location.origin}/graphql`, fetch: win.fetch.bind(win) });
      const result = await client.readUser({ id: '42' });
      expect(result.kind).to.equal('partial');
      expect(result.data.user.name).to.equal('Partial');
      expect(result.errors[0].path).to.deep.equal(['user', 'nickname']);
    });
    cy.wait('@ReadUser');
  });
});
