// Employees and the admin-managed earning-type catalogue.

import { h, emptyState } from '../dom.js';
import { openEmployeeDialog } from './employee-dialog.js';
import { openTypeDialog } from './type-dialog.js';

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
      const action = control.dataset.action;
      if (action === 'add-employee') openEmployeeDialog(ctx, {});
      else if (action === 'edit-employee') {
        const employee = ctx.findEmployee(control.dataset.employeeId);
        if (employee) openEmployeeDialog(ctx, { existing: employee });
      } else if (action === 'add-type') openTypeDialog(ctx, {});
      else if (action === 'edit-type') {
        const type = ctx.store.get().types.find((candidate) => candidate.id === control.dataset.typeId);
        if (type) openTypeDialog(ctx, { existing: type });
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
    const types = state.types
      .filter((type) => !type.deleted_at)
      .sort((a, b) => (a.sort_order ?? 0) - (b.sort_order ?? 0) || a.name.localeCompare(b.name));

    const head = h(
      'div',
      { class: 'view-head' },
      h(
        'div',
        {},
        h('h1', {}, 'Employees'),
        h('p', { class: 'subtitle' }, 'People and the earning types used on the day form'),
      ),
      h(
        'button',
        { class: 'btn btn-primary', type: 'button', 'data-action': 'add-employee' },
        'Add employee',
      ),
    );

    function employeeCard(employee) {
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
          h(
            'span',
            { class: 'card-meta' },
            employee.code ? `Code ${employee.code}` : 'No employee code',
          ),
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

    function typeCard(type) {
      return h(
        'button',
        {
          class: 'employee-card type-card',
          type: 'button',
          'data-action': 'edit-type',
          'data-type-id': type.id,
        },
        h(
          'span',
          { class: 'card-main' },
          h('span', { class: 'card-title' }, type.name),
          h(
            'span',
            { class: 'badge-row' },
            !type.active ? h('span', { class: 'badge badge-neutral' }, 'Inactive') : null,
          ),
        ),
        h('span', { class: 'card-chevron', 'aria-hidden': 'true' }, '›'),
      );
    }

    const sections = [];

    if (!state.employees.length) {
      sections.push(
        emptyState(
          'No employees yet',
          'Add employees, then record the business each person generates every day.',
          h(
            'button',
            { class: 'btn btn-primary', type: 'button', 'data-action': 'add-employee' },
            'Add your first employee',
          ),
        ),
      );
    } else {
      if (active.length) {
        sections.push(h('h2', { class: 'section-title' }, `Active (${active.length})`));
        sections.push(h('div', { class: 'card-list' }, active.map(employeeCard)));
      }
      if (inactive.length) {
        sections.push(h('h2', { class: 'section-title' }, `Inactive (${inactive.length})`));
        sections.push(h('div', { class: 'card-list' }, inactive.map(employeeCard)));
      }
    }

    sections.push(
      h(
        'div',
        { class: 'section-head' },
        h('h2', { class: 'section-title' }, 'Earning types'),
        h(
          'button',
          { class: 'btn btn-small', type: 'button', 'data-action': 'add-type' },
          'Add type',
        ),
      ),
    );
    if (!types.length) {
      sections.push(
        h(
          'div',
          { class: 'state state-empty' },
          h('h2', {}, 'No earning types'),
          h('p', {}, 'Add at least one type (for example Sales or Service) so the day form has rows to fill.'),
        ),
      );
    } else {
      sections.push(h('div', { class: 'card-list' }, types.map(typeCard)));
    }
    sections.push(
      h(
        'p',
        { class: 'field-hint' },
        'Types are managed on the server. They are cached on this device so day sheets can be entered offline.',
      ),
    );

    container.replaceChildren(head, ...sections);
  }

  paint();
  return () => {
    abort.abort();
    unsubscribe();
  };
}
