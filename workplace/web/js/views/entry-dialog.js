// Add/edit one employee-day: business rows per earning type + adjustments.

import { confirmDialog, h, openDialog, toast } from '../dom.js';
import { displayDate, formatDecimal, formatPaise, parsePaise } from '../format.js';
import { computeTotals } from '../totals.js';
import { deleteRow, saveEntry } from '../mutations.js';

const MAX_AMOUNT_MINOR = 100_000_000_000;
const MAX_DESCRIPTION_LEN = 300;

function readAmount(input) {
  const raw = input.value.trim();
  if (!raw) return { value: 0 };
  const paise = parsePaise(raw);
  if (paise === null) return { error: 'Enter an amount like 1250.50' };
  if (paise > MAX_AMOUNT_MINOR) return { error: 'Amount is too large' };
  return { value: paise };
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
  const types = ctx.store.get().types.filter((type) => type.active !== false);
  const existingByType = new Map(
    (existing?.lines ?? []).map((line) => [line.type_id, line]),
  );

  // Active types get an editable row. Lines whose type has since been renamed,
  // deactivated or deleted stay visible so an edit cannot silently drop money.
  const rows = [];
  const seen = new Set();
  for (const type of types) {
    const line = existingByType.get(type.id);
    rows.push({
      type_id: type.id,
      type_name: type.name,
      archived: false,
      description: line?.description ?? '',
      amount: line?.amount_minor ?? 0,
    });
    seen.add(type.id);
  }
  for (const line of existing?.lines ?? []) {
    if (seen.has(line.type_id)) continue;
    rows.push({
      type_id: line.type_id,
      type_name: line.type_name,
      archived: true,
      description: line.description,
      amount: line.amount_minor,
    });
    seen.add(line.type_id);
  }

  const rowControls = rows.map((row) => {
    const description = h('input', {
      type: 'text',
      autocomplete: 'off',
      maxlength: String(MAX_DESCRIPTION_LEN),
      value: row.description,
      placeholder: 'What was done? (optional)',
    });
    const amount = h('input', {
      type: 'text',
      inputmode: 'decimal',
      autocomplete: 'off',
      value: row.amount ? formatDecimal(row.amount) : '',
      placeholder: '0.00',
    });
    const amountError = h('p', { class: 'field-error', role: 'alert' });
    const element = h(
      'div',
      { class: 'earning-row' },
      h(
        'div',
        { class: 'earning-row-head' },
        h('strong', {}, row.type_name),
        row.archived ? h('span', { class: 'badge badge-neutral' }, 'Archived type') : null,
      ),
      h(
        'div',
        { class: 'earning-row-fields' },
        h(
          'label',
          { class: 'field' },
          h('span', { class: 'field-label' }, 'Description'),
          description,
        ),
        h(
          'label',
          { class: 'field field-amount' },
          h('span', { class: 'field-label' }, 'Amount (₹)'),
          amount,
        ),
      ),
      amountError,
    );
    return { ...row, description, amount, amountError, element };
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
    placeholder: 'Optional note for the day',
  });
  note.value = existing?.note ?? '';

  const bonusField = field('Bonus (₹)', bonus);
  const advanceField = field('Advance paid (₹)', advance, 'Deducted from the final money.');
  const deductionField = field('Deduction (₹)', deduction);
  const noteField = field('Note', note);
  const linesError = h('p', { class: 'field-error', role: 'alert' });

  const preview = h('div', { class: 'preview', 'aria-live': 'polite' });

  const typesSection = rows.length
    ? h('div', { class: 'earning-rows' }, rowControls.map((row) => row.element))
    : h(
        'div',
        { class: 'state state-empty' },
        h('h2', {}, 'No earning types yet'),
        h('p', {}, 'Add earning types (for example Sales, Service, Tips) in the Employees tab first.'),
      );

  const content = h(
    'div',
    { class: 'entry-form' },
    h(
      'p',
      { class: 'dialog-subtitle' },
      `${employee.name} · ${displayDate(date)} · business for the day`,
    ),
    typesSection,
    linesError,
    h(
      'div',
      { class: 'form-grid' },
      bonusField.wrapper,
      advanceField.wrapper,
      deductionField.wrapper,
    ),
    noteField.wrapper,
    h('div', { class: 'preview-wrap' }, h('h3', {}, 'Day total'), preview),
    h(
      'p',
      { class: 'field-hint' },
      'The server recalculates these amounts on sync; the preview is an estimate.',
    ),
  );

  function collect() {
    const errors = {};
    const lines = [];
    let anyAmount = false;
    rowControls.forEach((row, index) => {
      const amount = readAmount(row.amount);
      if (amount.error) {
        errors[`amount-${index}`] = amount.error;
      } else if (amount.value > 0) {
        anyAmount = true;
        lines.push({
          type_id: row.type_id,
          type_name: row.type_name,
          description: row.description.value.trim(),
          amount_minor: amount.value,
        });
      }
      if (row.description.value.trim().length > MAX_DESCRIPTION_LEN) {
        errors[`description-${index}`] = 'Description is too long';
      }
    });
    if (!anyAmount) errors.lines = 'Enter at least one earning amount.';

    const parsedAdjustments = {};
    for (const [key, input] of [
      ['bonus_minor', bonus],
      ['advance_minor', advance],
      ['deduction_minor', deduction],
    ]) {
      const amount = readAmount(input);
      if (amount.error) errors[key] = amount.error;
      else parsedAdjustments[key] = amount.value;
    }

    return { lines, adjustments: parsedAdjustments, errors };
  }

  function showErrors(errors) {
    linesError.textContent = errors.lines ?? '';
    rowControls.forEach((row, index) => {
      row.amountError.textContent =
        errors[`amount-${index}`] ?? errors[`description-${index}`] ?? '';
      row.amount.setAttribute('aria-invalid', errors[`amount-${index}`] ? 'true' : 'false');
      row.description.setAttribute(
        'aria-invalid',
        errors[`description-${index}`] ? 'true' : 'false',
      );
    });
    bonusField.error.textContent = errors.bonus_minor ?? '';
    advanceField.error.textContent = errors.advance_minor ?? '';
    deductionField.error.textContent = errors.deduction_minor ?? '';
  }

  function updatePreview() {
    const { lines, adjustments, errors } = collect();
    if (errors.lines || Object.keys(errors).length) {
      preview.replaceChildren(
        h('p', { class: 'muted' }, 'Fix the highlighted fields to see the estimate.'),
      );
      return;
    }
    const totals = computeTotals(lines, adjustments);
    preview.replaceChildren(
      h(
        'dl',
        { class: 'preview-grid' },
        h('dt', { class: 'strong' }, 'Business'),
        h('dd', { class: 'strong' }, formatPaise(totals.business_minor)),
        h('dt', {}, 'Bonus'),
        h('dd', {}, formatPaise(totals.bonus_minor)),
        h('dt', { class: 'strong' }, 'Gross'),
        h('dd', { class: 'strong' }, formatPaise(totals.gross_minor)),
        h('dt', {}, 'Advance'),
        h('dd', {}, `− ${formatPaise(totals.advance_minor)}`),
        h('dt', {}, 'Deduction'),
        h('dd', {}, `− ${formatPaise(totals.deduction_minor)}`),
        h('dt', { class: 'strong' }, 'Final money'),
        h('dd', { class: 'strong net' }, formatPaise(totals.net_minor)),
      ),
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
          title: 'Delete this day sheet?',
          message: `Remove ${employee.name}'s earnings for ${displayDate(date)}? The deletion syncs to all devices.`,
          confirmLabel: 'Delete day',
          danger: true,
        });
        if (!confirmed) return false;
        await deleteRow('entry', existing, { conflict: ctx.findConflict('entry', existing.id) });
        ctx.refresh();
        void ctx.sync.run();
        toast('Day deleted', 'success');
      },
    });
  }
  actions.push({
    label: existing ? 'Save changes' : 'Save day',
    kind: 'primary',
    onClick: async () => {
      const { lines, adjustments, errors } = collect();
      showErrors(errors);
      if (Object.keys(errors).length) {
        toast('Please fix the highlighted fields', 'error');
        return false;
      }

      const duplicate = ctx.state.entries.find(
        (entry) =>
          entry.employee_id === employee.id &&
          entry.work_date === date &&
          !entry.deleted_at &&
          entry.id !== existing?.id,
      );
      if (duplicate) {
        toast('This employee already has a day sheet for that date', 'error');
        return false;
      }

      await saveEntry({
        employee,
        date,
        existing,
        lines,
        adjustments,
        note: note.value.trim(),
        conflict: ctx.findConflict('entry', existing?.id),
      });
      ctx.refresh();
      void ctx.sync.run();
      toast(existing ? 'Day updated' : 'Day saved', 'success');
    },
  });

  openDialog({
    title: existing ? 'Edit day earnings' : 'Add day earnings',
    description: conflict ? 'This day has a sync conflict. Saving keeps your version.' : null,
    content,
    actions,
  });
}
