// Add/edit earning types. Types are server-side configuration; managing them
// needs a connection. Phones cache the list for offline day-sheet entry.

import { confirmDialog, h, openDialog, toast } from '../dom.js';
import * as db from '../db.js';

export function openTypeDialog(ctx, { existing = null } = {}) {
  const name = h('input', {
    type: 'text',
    autocomplete: 'off',
    maxlength: '60',
    value: existing?.name ?? '',
    placeholder: 'e.g. Sales, Service, Tips',
  });
  const active = h('input', { type: 'checkbox', checked: existing?.active !== false });
  const nameError = h('p', { class: 'field-error', role: 'alert' });

  const content = h(
    'div',
    { class: 'employee-form' },
    h(
      'div',
      { class: 'field' },
      h('label', {}, h('span', { class: 'field-label' }, 'Type name'), name),
      h(
        'p',
        { class: 'field-hint' },
        'One amount per type per employee-day. Historical sheets keep the name even if you rename this later.',
      ),
      nameError,
    ),
    h(
      'label',
      { class: 'field checkbox' },
      active,
      h('span', {}, 'Active — show on the day form'),
    ),
  );

  function requireOnline() {
    if (!ctx.store.get().online) {
      toast('Earning types are managed on the server. Reconnect to change them.', 'error');
      return false;
    }
    return true;
  }

  const actions = [];
  if (existing) {
    actions.push({
      label: 'Delete',
      kind: 'danger',
      onClick: async () => {
        if (!requireOnline()) return false;
        const confirmed = await confirmDialog({
          title: 'Delete this earning type?',
          message: `"${existing.name}" will disappear from new day sheets. Past entries and reports keep the recorded name and amounts.`,
          confirmLabel: 'Delete type',
          danger: true,
        });
        if (!confirmed) return false;
        try {
          await ctx.api.deleteEarningType(existing.id);
          await db.remove(db.STORES.types, existing.id);
          await ctx.refreshTypes();
          toast('Earning type deleted', 'success');
        } catch (error) {
          toast(error?.message ?? 'Could not delete the type', 'error');
        }
      },
    });
  }
  actions.push({
    label: existing ? 'Save changes' : 'Add type',
    kind: 'primary',
    onClick: async () => {
      if (!requireOnline()) return false;
      const trimmed = name.value.trim();
      if (!trimmed) {
        nameError.textContent = 'Name is required';
        name.setAttribute('aria-invalid', 'true');
        return false;
      }
      if (trimmed.length > 60) {
        nameError.textContent = 'Name is too long';
        name.setAttribute('aria-invalid', 'true');
        return false;
      }

      const clash = ctx.store
        .get()
        .types.find(
          (type) =>
            type.name.toLowerCase() === trimmed.toLowerCase() && type.id !== existing?.id,
        );
      if (clash) {
        nameError.textContent = 'This type name already exists';
        return false;
      }

      try {
        const payload = { name: trimmed, active: active.checked };
        const row = existing
          ? await ctx.api.updateEarningType(existing.id, payload)
          : await ctx.api.createEarningType(payload);
        await db.put(db.STORES.types, row);
        await ctx.refreshTypes();
        toast(existing ? 'Earning type updated' : 'Earning type added', 'success');
      } catch (error) {
        if (error?.code === 'duplicate_type_name') {
          nameError.textContent = 'This type name already exists';
          name.setAttribute('aria-invalid', 'true');
        } else {
          toast(error?.message ?? 'Could not save the type', 'error');
        }
        return false;
      }
    },
  });

  openDialog({
    title: existing ? 'Edit earning type' : 'Add earning type',
    content,
    actions,
  });
}
