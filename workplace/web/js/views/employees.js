// Employee list and pay-rule management.

import { h, emptyState } from '../dom.js';
import { basisLabel, rateLabel } from '../pay.js';
import { openEmployeeDialog } from './employee-dialog.js';

export function renderEmployees(root, ctx) {
  const container = h('section', { class: 'view' });
  root.replaceChildren(container);
  const abort = new AbortController();
  const unsubscribe = ctx.store.subscribe(() => paint());

  container.addEventListener(
    'click',
    (event) => {
      const control = event.target.closest('[data-action]');
      if (!control) return;
      if (control.dataset.action === 'add-employee') openEmployeeDialog(ctx, {});
      else if (control.dataset.action === 'edit-employee') {
        const employee = ctx.findEmployee(control.dataset.employeeId);
        if (employee) openEmployeeDialog(ctx, { existing: employee });
      }
    },
    { signal: abort.signal },
  );

  function paint() {
    const state = ctx.store.get();
    const active = state.employees
      .filter((employee) => employee.active)
      .sort((a, b) => a.name.localeCompare(b.name, 'en', { sensitivity: 'base' }));
    const inactive = state.employees
      .filter((employee) => !employee.active)
      .sort((a, b) => a.name.localeCompare(b.name, 'en', { sensitivity: 'base' }));

    const head = h(
      'div',
      { class: 'view-head' },
      h(
        'div',
        {},
        h('h1', {}, 'Employees'),
        h('p', { class: 'subtitle' }, 'Pay rules, rates and overtime settings'),
      ),
      h(
        'button',
        { class: 'btn btn-primary', type: 'button', 'data-action': 'add-employee' },
        'Add employee',
      ),
    );

    if (!state.employees.length) {
      container.replaceChildren(
        head,
        emptyState(
          'No employees yet',
          'Add employees with their pay basis and rate. You can change rates later without altering past pay.',
          h(
            'button',
            { class: 'btn btn-primary', type: 'button', 'data-action': 'add-employee' },
            'Add your first employee',
          ),
        ),
      );
      return;
    }

    function card(employee) {
      const pending = ctx.isPending('employee', employee.id);
      const conflict = ctx.findConflict('employee', employee.id);
      return h(
        'button',
        {
          class: 'employee-card',
          type: 'button',
          'data-action': 'edit-employee',
          'data-employee-id': employee.id,
        },
        h(
          'span',
          { class: 'card-main' },
          h('span', { class: 'card-title' }, employee.name),
          h('span', { class: 'card-meta' }, `${basisLabel(employee.pay_basis)} · ${rateLabel(employee)}`),
          employee.code ? h('span', { class: 'card-code' }, employee.code) : null,
          h(
            'span',
            { class: 'badge-row' },
            pending ? h('span', { class: 'badge badge-pending' }, 'Pending') : null,
            conflict ? h('span', { class: 'badge badge-warn' }, 'Conflict') : null,
            !employee.active ? h('span', { class: 'badge badge-neutral' }, 'Inactive') : null,
          ),
        ),
        h('span', { class: 'card-chevron', 'aria-hidden': 'true' }, '›'),
      );
    }

    const sections = [];
    if (active.length) {
      sections.push(h('h2', { class: 'section-title' }, `Active (${active.length})`));
      sections.push(h('div', { class: 'card-list' }, active.map(card)));
    }
    if (inactive.length) {
      sections.push(h('h2', { class: 'section-title' }, `Inactive (${inactive.length})`));
      sections.push(h('div', { class: 'card-list' }, inactive.map(card)));
    }

    container.replaceChildren(head, ...sections);
  }

  paint();
  return () => {
    abort.abort();
    unsubscribe();
  };
}
