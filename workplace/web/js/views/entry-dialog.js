// Add/edit dialog for one employee-day entry.

import { confirmDialog, h, openDialog, toast } from '../dom.js';
import {
  displayDate,
  formatDecimal,
  formatHours,
  formatPaise,
  fractionLabel,
  parseHoursToMinutes,
  parsePaise,
} from '../format.js';
import { basisLabel, computeEntry, rateLabel } from '../pay.js';
import { deleteRow, saveEntry } from '../mutations.js';

const MAX_AMOUNT_MINOR = 100_000_000_000;

function readAmount(input, { required = false, fallback = 0 } = {}) {
  const raw = input.value.trim();
  if (!raw) return required ? { error: 'Required' } : { value: fallback };
  const paise = parsePaise(raw);
  if (paise === null) return { error: 'Enter a valid amount like 1250.50' };
  if (paise > MAX_AMOUNT_MINOR) return { error: 'Amount is too large' };
  return { value: paise };
}

function readHours(input) {
  const raw = input.value.trim();
  if (!raw) return { value: 0 };
  const minutes = parseHoursToMinutes(raw);
  if (minutes === null) return { error: 'Enter hours like 7.5 (max 24)' };
  return { value: minutes };
}

function field(labelText, control, hintText = null) {
  const error = h('p', { class: 'field-error', role: 'alert' });
  const wrapper = h(
    'div',
    { class: 'field' },
    h('label', {}, h('span', { class: 'field-label' }, labelText), control),
    hintText ? h('p', { class: 'field-hint' }, hintText) : null,
    error,
  );
  return { wrapper, control, error };
}

export function openEntryDialog(ctx, { employee, date, existing = null }) {
  const conflict = existing ? ctx.findConflict('entry', existing.id) : null;

  const dayFraction = h(
    'select',
    { name: 'day_fraction' },
    [
      [5000, 'Half day (0.5)'],
      [10000, 'Full day (1.0)'],
      [15000, '1.5 days'],
      [20000, '2 days'],
    ].map(([value, label]) =>
      h('option', { value: String(value), selected: existing?.day_fraction_bp === value }, label),
    ),
  );
  const regularHours = h('input', {
    type: 'text',
    inputmode: 'decimal',
    autocomplete: 'off',
    value: existing ? formatHours(existing.regular_minutes) : '8',
  });
  const overtimeHours = h('input', {
    type: 'text',
    inputmode: 'decimal',
    autocomplete: 'off',
    value: existing ? formatHours(existing.overtime_minutes) : '0',
  });
  const fixedAmount = h('input', {
    type: 'text',
    inputmode: 'decimal',
    autocomplete: 'off',
    value: formatDecimal(existing?.override_amount_minor ?? employee.rate_minor),
  });
  const overrideAmount = h('input', {
    type: 'text',
    inputmode: 'decimal',
    autocomplete: 'off',
    placeholder: 'Use employee rate',
    value: existing?.override_amount_minor != null ? formatDecimal(existing.override_amount_minor) : '',
  });
  const bonus = h('input', {
    type: 'text',
    inputmode: 'decimal',
    autocomplete: 'off',
    value: existing ? formatDecimal(existing.bonus_minor) : '0',
  });
  const advance = h('input', {
    type: 'text',
    inputmode: 'decimal',
    autocomplete: 'off',
    value: existing ? formatDecimal(existing.advance_minor) : '0',
  });
  const deduction = h('input', {
    type: 'text',
    inputmode: 'decimal',
    autocomplete: 'off',
    value: existing ? formatDecimal(existing.deduction_minor) : '0',
  });
  const note = h('textarea', {
    rows: '2',
    maxlength: '2000',
    placeholder: 'Optional note',
  });
  note.value = existing?.note ?? '';

  const dayFractionField = field('Day worked', dayFraction, 'Half days and extra days are supported.');
  const regularField = field('Regular hours', regularHours, 'Example: 7.5');
  const overtimeField = field(
    'Overtime hours',
    overtimeHours,
    `Paid at ${((employee.ot_multiplier_bp ?? 15000) / 10000).toFixed(2)}× the normal rate.`,
  );
  const fixedField = field('Amount for this entry (₹)', fixedAmount, 'Defaults to the employee fixed rate.');
  const overrideField = field(
    'Override base pay (₹)',
    overrideAmount,
    'Leave blank to use the employee rate.',
  );
  const bonusField = field('Bonus (₹)', bonus);
  const advanceField = field('Advance paid (₹)', advance, 'Deducted from today’s payout.');
  const deductionField = field('Deduction (₹)', deduction);
  const noteField = field('Note', note);

  const preview = h('div', { class: 'preview', 'aria-live': 'polite' });

  const basisSection = h('div', { class: 'form-grid' });
  if (employee.pay_basis === 'day_rate') {
    basisSection.append(dayFractionField.wrapper, overtimeField.wrapper);
  } else if (employee.pay_basis === 'hourly') {
    basisSection.append(regularField.wrapper, overtimeField.wrapper);
  } else {
    basisSection.append(fixedField.wrapper);
  }

  const moneySection = h(
    'div',
    { class: 'form-grid' },
    bonusField.wrapper,
    advanceField.wrapper,
    deductionField.wrapper,
  );

  const advanced = h(
    'details',
    { class: 'advanced' },
    h('summary', {}, 'More options'),
    employee.pay_basis === 'fixed' ? null : overrideField.wrapper,
    noteField.wrapper,
  );
  if (employee.pay_basis === 'fixed') advanced.append(noteField.wrapper);

  const content = h(
    'div',
    { class: 'entry-form' },
    h(
      'p',
      { class: 'dialog-subtitle' },
      `${employee.name} · ${displayDate(date)} · ${basisLabel(employee.pay_basis)} · ${rateLabel(employee)}`,
    ),
    basisSection,
    moneySection,
    advanced,
    h('div', { class: 'preview-wrap' }, h('h3', {}, 'Preview'), preview),
    h('p', { class: 'field-hint' }, 'Preview is an estimate; the server recalculates amounts on sync.'),
  );

  function collect() {
    const errors = {};
    const amounts = {
      day_fraction_bp: employee.pay_basis === 'day_rate' ? Number(dayFraction.value) : 10000,
      regular_minutes: 0,
      overtime_minutes: 0,
      ot_multiplier_bp: null,
      override_amount_minor: null,
      bonus_minor: 0,
      advance_minor: 0,
      deduction_minor: 0,
    };

    if (employee.pay_basis === 'hourly') {
      const regular = readHours(regularHours);
      if (regular.error) errors.regular = regular.error;
      else amounts.regular_minutes = regular.value;
    }
    if (employee.pay_basis !== 'fixed') {
      const overtime = readHours(overtimeHours);
      if (overtime.error) errors.overtime = overtime.error;
      else amounts.overtime_minutes = overtime.value;
    }

    if (employee.pay_basis === 'fixed') {
      const amount = readAmount(fixedAmount, { required: true, fallback: employee.rate_minor });
      if (amount.error) errors.fixed = amount.error;
      else amounts.override_amount_minor = amount.value;
    } else {
      const override = readAmount(overrideAmount, { fallback: null });
      if (override.error) errors.override = override.error;
      else amounts.override_amount_minor = override.value;
    }

    for (const [key, input, target] of [
      ['bonus', bonus, 'bonus_minor'],
      ['advance', advance, 'advance_minor'],
      ['deduction', deduction, 'deduction_minor'],
    ]) {
      const amount = readAmount(input);
      if (amount.error) errors[key] = amount.error;
      else amounts[target] = amount.value;
    }

    return { amounts, errors };
  }

  function showErrors(errors) {
    const map = [
      [regularField, errors.regular],
      [overtimeField, errors.overtime],
      [fixedField, errors.fixed],
      [overrideField, errors.override],
      [bonusField, errors.bonus],
      [advanceField, errors.advance],
      [deductionField, errors.deduction],
    ];
    for (const [target, message] of map) target.error.textContent = message ?? '';
    for (const target of [regularHours, overtimeHours, fixedAmount, overrideAmount, bonus, advance, deduction]) {
      target.setAttribute('aria-invalid', target.getAttribute('aria-invalid') === 'true' ? 'false' : target.getAttribute('aria-invalid') ?? 'false');
    }
  }

  function updatePreview() {
    const { amounts, errors } = collect();
    if (Object.keys(errors).length) {
      preview.replaceChildren(h('p', { class: 'muted' }, 'Fix the highlighted fields to see the estimate.'));
      return;
    }
    const computed = computeEntry(employee, amounts);
    preview.replaceChildren(
      h(
        'dl',
        { class: 'preview-grid' },
        h('dt', {}, 'Base'),
        h('dd', {}, formatPaise(computed.base_minor)),
        h('dt', {}, 'Overtime'),
        h('dd', {}, formatPaise(computed.ot_minor)),
        h('dt', {}, 'Bonus'),
        h('dd', {}, formatPaise(computed.bonus_minor)),
        h('dt', { class: 'strong' }, 'Gross'),
        h('dd', { class: 'strong' }, formatPaise(computed.gross_minor)),
        h('dt', {}, 'Advance'),
        h('dd', {}, `− ${formatPaise(computed.advance_minor)}`),
        h('dt', {}, 'Deduction'),
        h('dd', {}, `− ${formatPaise(computed.deduction_minor)}`),
        h('dt', { class: 'strong' }, 'Net'),
        h('dd', { class: 'strong net' }, formatPaise(computed.net_minor)),
      ),
      h('p', { class: 'preview-hours' }, `${formatHours(computed.regular_minutes)} reg · ${formatHours(computed.overtime_minutes)} OT`),
    );
  }

  content.addEventListener('input', updatePreview);
  content.addEventListener('change', updatePreview);
  updatePreview();

  const actions = [];
  if (existing) {
    actions.push({
      label: 'Delete',
      kind: 'danger',
      onClick: async () => {
        const confirmed = await confirmDialog({
          title: 'Delete this entry?',
          message: `Remove ${employee.name}'s entry for ${displayDate(date)}? The deletion syncs to all devices.`,
          confirmLabel: 'Delete entry',
          danger: true,
        });
        if (!confirmed) return false;
        await deleteRow('entry', existing, { conflict: ctx.findConflict('entry', existing.id) });
        ctx.refresh();
        void ctx.sync.run();
        toast('Entry deleted', 'success');
      },
    });
  }
  actions.push({
    label: existing ? 'Save changes' : 'Save entry',
    kind: 'primary',
    onClick: async () => {
      const { amounts, errors } = collect();
      showErrors(errors);
      if (Object.keys(errors).length) {
        toast('Please fix the highlighted fields', 'error');
        return false;
      }

      // Local duplicate guard: the server also enforces this.
      const duplicate = ctx.state.entries.find(
        (entry) =>
          entry.employee_id === employee.id &&
          entry.work_date === date &&
          !entry.deleted_at &&
          entry.id !== existing?.id,
      );
      if (duplicate) {
        toast('This employee already has an entry for that day', 'error');
        return false;
      }

      await saveEntry({
        employee,
        date,
        existing,
        amounts,
        note: note.value.trim(),
        conflict: ctx.findConflict('entry', existing?.id),
      });
      ctx.refresh();
      void ctx.sync.run();
      toast(existing ? 'Entry updated' : 'Entry saved', 'success');
    },
  });

  openDialog({
    title: existing ? 'Edit entry' : 'Add entry',
    description: conflict ? 'This entry has a sync conflict. Saving will keep your version.' : null,
    content,
    actions,
  });
}
