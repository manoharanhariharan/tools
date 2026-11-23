// Add/edit dialog for an employee pay rule.

import { confirmDialog, h, openDialog, toast } from '../dom.js';
import { formatDecimal, parseHoursToMinutes, parsePaise } from '../format.js';
import { saveEmployee, deleteRow } from '../mutations.js';

const BASIS_OPTIONS = [
  ['day_rate', 'Day rate — paid per day, overtime from a standard day'],
  ['hourly', 'Hourly — paid for regular hours, overtime at a multiplier'],
  ['fixed', 'Fixed per entry — one amount per day/entry'],
];

export function openEmployeeDialog(ctx, { existing = null } = {}) {
  const conflict = existing ? ctx.findConflict('employee', existing.id) : null;

  const name = h('input', {
    type: 'text',
    autocomplete: 'name',
    maxlength: '120',
    required: true,
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
  const basis = h(
    'select',
    {},
    BASIS_OPTIONS.map(([value, label]) =>
      h('option', { value, selected: (existing?.pay_basis ?? 'day_rate') === value }, label),
    ),
  );
  const rate = h('input', {
    type: 'text',
    inputmode: 'decimal',
    autocomplete: 'off',
    value: formatDecimal(existing?.rate_minor ?? 80000),
  });
  const multiplier = h('input', {
    type: 'text',
    inputmode: 'decimal',
    autocomplete: 'off',
    maxlength: '5',
    value: ((existing?.ot_multiplier_bp ?? 15000) / 10000).toFixed(2),
  });
  const standardHours = h('input', {
    type: 'text',
    inputmode: 'decimal',
    autocomplete: 'off',
    value: existing ? (existing.standard_minutes_per_day / 60).toFixed(2) : '8.00',
  });
  const note = h('textarea', { rows: '2', maxlength: '2000', placeholder: 'Optional note' });
  note.value = existing?.note ?? '';
  const active = h('input', { type: 'checkbox', checked: existing?.active !== false });

  const nameError = h('p', { class: 'field-error', role: 'alert' });
  const codeError = h('p', { class: 'field-error', role: 'alert' });
  const rateError = h('p', { class: 'field-error', role: 'alert' });
  const multiplierError = h('p', { class: 'field-error', role: 'alert' });
  const standardError = h('p', { class: 'field-error', role: 'alert' });

  const rateLabel = h('span', { class: 'field-label' }, 'Day rate (₹)');
  const standardField = h(
    'div',
    { class: 'field' },
    h('label', {}, h('span', { class: 'field-label' }, 'Standard hours per day'), standardHours),
    h('p', { class: 'field-hint' }, 'Used to price overtime for day-rate employees.'),
    standardError,
  );

  function syncBasisLabels() {
    const value = basis.value;
    rateLabel.textContent =
      value === 'hourly' ? 'Hourly rate (₹)' : value === 'fixed' ? 'Amount per entry (₹)' : 'Day rate (₹)';
    standardField.hidden = value !== 'day_rate';
    multiplierField.hidden = value === 'fixed';
  }

  const multiplierField = h(
    'div',
    { class: 'field' },
    h('label', {}, h('span', { class: 'field-label' }, 'Overtime multiplier'), multiplier),
    h('p', { class: 'field-hint' }, '1.50 = time and a half. Between 1.00 and 3.00.'),
    multiplierError,
  );

  basis.addEventListener('change', syncBasisLabels);

  const content = h(
    'div',
    { class: 'employee-form' },
    h(
      'div',
      { class: 'form-grid' },
      h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'Name'), name), nameError),
      h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'Employee code'), code), codeError),
    ),
    h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'Pay basis'), basis)),
    h(
      'div',
      { class: 'form-grid' },
      h('div', { class: 'field' }, h('label', {}, rateLabel, rate), rateError),
      multiplierField,
    ),
    standardField,
    h(
      'label',
      { class: 'field checkbox' },
      active,
      h('span', {}, 'Active — can be given new entries'),
    ),
    h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'Note'), note)),
    h(
      'p',
      { class: 'field-hint' },
      'Rate edits apply to new entries only. Existing entries keep the amounts saved with them.',
    ),
  );

  syncBasisLabels();

  function collect() {
    const errors = {};
    const trimmedName = name.value.trim();
    if (!trimmedName) errors.name = 'Name is required';
    else if (trimmedName.length > 120) errors.name = 'Name is too long';

    const trimmedCode = code.value.trim();
    if (trimmedCode && !/^[A-Za-z0-9._/-]{1,32}$/.test(trimmedCode)) {
      errors.code = 'Use letters, digits, dot, dash, underscore or slash (max 32)';
    }

    const rateMinor = parsePaise(rate.value);
    if (rateMinor === null) errors.rate = 'Enter a valid amount';
    else if (rateMinor > 100_000_000_000) errors.rate = 'Amount is too large';

    let multiplierBp = existing?.ot_multiplier_bp ?? 15000;
    const multiplierRaw = multiplier.value.trim();
    if (basis.value !== 'fixed') {
      const match = /^(\d)(?:\.(\d{0,2}))?$/.exec(multiplierRaw);
      if (!match) {
        errors.multiplier = 'Enter a value like 1.5';
      } else {
        multiplierBp = Number(match[1]) * 10000 + Number((match[2] ?? '').padEnd(2, '0'));
        if (multiplierBp < 10000 || multiplierBp > 30000) errors.multiplier = 'Must be between 1.00 and 3.00';
      }
    }

    let standardMinutes = existing?.standard_minutes_per_day ?? 480;
    if (basis.value === 'day_rate') {
      const minutes = parseHoursToMinutes(standardHours.value);
      if (minutes === null || minutes < 60) errors.standard = 'Enter hours between 1 and 24';
      else standardMinutes = minutes;
    }

    return {
      errors,
      input: {
        name: trimmedName,
        code: trimmedCode || null,
        pay_basis: basis.value,
        rate_minor: rateMinor ?? 0,
        ot_multiplier_bp: multiplierBp,
        standard_minutes_per_day: standardMinutes,
        note: note.value.trim(),
        active: active.checked,
      },
    };
  }

  const actions = [];
  if (existing) {
    actions.push({
      label: 'Delete',
      kind: 'danger',
      onClick: async () => {
        const confirmed = await confirmDialog({
          title: 'Delete this employee?',
          message: `${existing.name} will be hidden from the entry screen. Their past entries and reports are kept.`,
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
      rateError.textContent = errors.rate ?? '';
      multiplierError.textContent = errors.multiplier ?? '';
      standardError.textContent = errors.standard ?? '';
      for (const [control, error] of [
        [name, errors.name],
        [code, errors.code],
        [rate, errors.rate],
        [multiplier, errors.multiplier],
        [standardHours, errors.standard],
      ]) {
        control.setAttribute('aria-invalid', error ? 'true' : 'false');
      }
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
