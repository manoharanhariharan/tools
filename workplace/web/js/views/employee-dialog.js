// Add/edit an employee. Day earnings are entered separately; employees no
// longer carry a rate, basis or hours.

import { confirmDialog, h, openDialog, toast } from '../dom.js';
import { saveEmployee, deleteRow } from '../mutations.js';

export function openEmployeeDialog(ctx, { existing = null } = {}) {
  const conflict = existing ? ctx.findConflict('employee', existing.id) : null;

  const name = h('input', {
    type: 'text',
    autocomplete: 'name',
    maxlength: '120',
    value: existing?.name ?? '',
    placeholder: 'Employee name',
  });
  const code = h('input', {
    type: 'text',
    autocomplete: 'off',
    maxlength: '32',
    value: existing?.code ?? '',
    placeholder: 'Optional, e.g. EMP-01',
  });
  const note = h('textarea', { rows: '2', maxlength: '2000', placeholder: 'Optional note' });
  note.value = existing?.note ?? '';
  const active = h('input', { type: 'checkbox', checked: existing?.active !== false });

  const nameError = h('p', { class: 'field-error', role: 'alert' });
  const codeError = h('p', { class: 'field-error', role: 'alert' });
  const pin = h('input', {
    type: 'password',
    inputmode: 'numeric',
    autocomplete: 'off',
    maxlength: '4',
    pattern: '[0-9]{4}',
    placeholder: '4-digit PIN',
  });
  const pinError = h('p', { class: 'field-error', role: 'alert' });
  const pinStatus = h('span', { class: 'badge badge-neutral' }, existing ? '…' : 'set after saving');

  const pinSection = h(
    'details',
    { class: 'advanced' },
    h('summary', {}, 'Employee login PIN'),
    h(
      'div',
      { class: 'field' },
      h('label', {}, h('span', { class: 'field-label' }, 'New PIN (optional)'), pin),
      h('p', { class: 'field-hint' }, `Employees sign in with their code and this 4-digit PIN. ${existing ? 'Leave blank to keep the current PIN.' : 'Save the employee first, then reopen to set a PIN.'}`),
      pinError,
      h('div', { class: 'badge-row', style: { marginTop: '6px' } }, pinStatus),
    ),
  );

  if (existing) {
    ctx.api
      .employeePinStatus(existing.id)
      .then((status) => {
        pinStatus.textContent = status?.has_pin ? 'PIN is set' : 'No PIN';
        pinStatus.className = `badge ${status?.has_pin ? 'badge-pending' : 'badge-neutral'}`;
      })
      .catch(() => {
        pinStatus.textContent = 'PIN status unknown';
      });
  }

  const content = h(
    'div',
    { class: 'employee-form' },
    h(
      'div',
      { class: 'form-grid' },
      h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'Name'), name), nameError),
      h(
        'div',
        { class: 'field' },
        h('label', {}, h('span', { class: 'field-label' }, 'Employee code'), code),
        codeError,
      ),
    ),
    h(
      'label',
      { class: 'field checkbox' },
      active,
      h('span', {}, 'Active — can be given new day sheets'),
    ),
    h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'Note'), note)),
    existing ? pinSection : null,
  );

  function collect() {
    const errors = {};
    const trimmedName = name.value.trim();
    if (!trimmedName) errors.name = 'Name is required';
    else if (trimmedName.length > 120) errors.name = 'Name is too long';

    const trimmedCode = code.value.trim();
    if (trimmedCode && !/^[A-Za-z0-9._/-]{1,32}$/.test(trimmedCode)) {
      errors.code = 'Use letters, digits, dot, dash, underscore or slash (max 32)';
    }

    return {
      errors,
      input: {
        name: trimmedName,
        code: trimmedCode || null,
        note: note.value.trim(),
        active: active.checked,
      },
    };
  }

  const actions = [];
  if (existing) {
    actions.push({
      label: 'Remove PIN',
      onClick: async () => {
        if (!ctx.state.online) {
          toast('Removing the PIN needs a connection', 'error');
          return false;
        }
        try {
          await ctx.api.clearEmployeePin(existing.id);
          pin.value = '';
          pinStatus.textContent = 'No PIN';
          pinStatus.className = 'badge badge-neutral';
          toast('Employee PIN removed', 'success');
        } catch (error) {
          toast(error?.message ?? 'Could not remove the PIN', 'error');
          return false;
        }
      },
    });
    actions.push({
      label: 'Delete',
      kind: 'danger',
      onClick: async () => {
        const confirmed = await confirmDialog({
          title: 'Delete this employee?',
          message: `${existing.name} will be hidden from the entry screen. Their past day sheets and reports are kept.`,
          confirmLabel: 'Delete employee',
          danger: true,
        });
        if (!confirmed) return false;
        await deleteRow('employee', existing, {
          conflict: ctx.findConflict('employee', existing.id),
        });
        ctx.refresh();
        void ctx.sync.run();
        toast('Employee deleted', 'success');
      },
    });
  }
  actions.push({
    label: existing ? 'Save changes' : 'Add employee',
    kind: 'primary',
    onClick: async () => {
      const { errors, input } = collect();
      nameError.textContent = errors.name ?? '';
      codeError.textContent = errors.code ?? '';
      name.setAttribute('aria-invalid', errors.name ? 'true' : 'false');
      code.setAttribute('aria-invalid', errors.code ? 'true' : 'false');
      if (Object.keys(errors).length) {
        toast('Please fix the highlighted fields', 'error');
        return false;
      }

      if (input.code) {
        const clash = ctx.state.allEmployees.find(
          (employee) =>
            !employee.deleted_at &&
            employee.code?.toLowerCase() === input.code.toLowerCase() &&
            employee.id !== existing?.id,
        );
        if (clash) {
          codeError.textContent = `Already used by ${clash.name}`;
          code.setAttribute('aria-invalid', 'true');
          return false;
        }
      }

      await saveEmployee(input, {
        existing,
        conflict: ctx.findConflict('employee', existing?.id),
      });

      if (existing && pin.value) {
        if (!/^\d{4}$/.test(pin.value)) {
          pinError.textContent = 'PIN must be exactly 4 digits';
          pin.setAttribute('aria-invalid', 'true');
          return false;
        }
        if (!ctx.state.online) {
          pinError.textContent = 'Setting a PIN needs a connection. Reconnect and try again.';
          return false;
        }
        try {
          await ctx.api.setEmployeePin(existing.id, pin.value);
          pinStatus.textContent = 'PIN is set';
          pinStatus.className = 'badge badge-pending';
        } catch (error) {
          pinError.textContent = error?.message ?? 'Could not set the PIN';
          return false;
        }
      }

      ctx.refresh();
      void ctx.sync.run();
      toast(existing ? 'Employee updated' : 'Employee added', 'success');
    },
  });

  openDialog({
    title: existing ? 'Edit employee' : 'Add employee',
    description: conflict ? 'This employee has a sync conflict. Saving keeps your version.' : null,
    content,
    actions,
  });
}
