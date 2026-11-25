// Sign-in screen with an Admin tab and an Employee tab (employee code + PIN).
// Sessions are server-side cookies; the PWA holds no tokens.

import { h, spinner } from '../dom.js';

export function renderLogin(root, ctx) {
  const abort = new AbortController();
  const error = h('p', { class: 'form-error', role: 'alert' });

  const adminPassword = h('input', {
    type: 'password',
    name: 'password',
    autocomplete: 'current-password',
    required: true,
    minlength: '10',
  });

  const employeeCode = h('input', {
    type: 'text',
    name: 'code',
    autocomplete: 'username',
    autocapitalize: 'characters',
    spellcheck: 'false',
    placeholder: 'e.g. EMP-01',
    required: true,
  });
  const employeePin = h('input', {
    type: 'password',
    name: 'pin',
    inputmode: 'numeric',
    autocomplete: 'current-password',
    maxlength: '4',
    pattern: '[0-9]{4}',
    required: true,
  });

  const adminForm = h(
    'form',
    { class: 'login-card', novalidate: true, 'data-form': 'admin' },
    h('h1', {}, 'Admin sign in'),
    h('p', { class: 'muted' }, 'Use the admin password for this Daybook server.'),
    h('input', {
      type: 'text',
      name: 'username',
      autocomplete: 'username',
      value: 'admin',
      readonly: true,
      tabindex: '-1',
      class: 'visually-hidden',
      'aria-hidden': 'true',
    }),
    h(
      'div',
      { class: 'field' },
      h('label', {}, h('span', { class: 'field-label' }, 'Password'), adminPassword),
    ),
    h('button', { class: 'btn btn-primary btn-block', type: 'submit' }, 'Sign in'),
  );

  const employeeForm = h(
    'form',
    { class: 'login-card', novalidate: true, 'data-form': 'employee', hidden: true },
    h('h1', {}, 'Employee sign in'),
    h('p', { class: 'muted' }, 'Enter your employee code and the 4-digit PIN your admin set.'),
    h(
      'div',
      { class: 'field' },
      h('label', {}, h('span', { class: 'field-label' }, 'Employee code'), employeeCode),
    ),
    h(
      'div',
      { class: 'field' },
      h('label', {}, h('span', { class: 'field-label' }, 'PIN'), employeePin),
    ),
    h('button', { class: 'btn btn-primary btn-block', type: 'submit' }, 'Sign in'),
  );

  const tabs = h(
    'div',
    { class: 'login-tabs', role: 'tablist', 'aria-label': 'Sign in as' },
    h(
      'button',
      { class: 'login-tab login-tab-active', type: 'button', role: 'tab', 'aria-selected': 'true', 'data-tab': 'admin' },
      'Admin',
    ),
    h(
      'button',
      { class: 'login-tab', type: 'button', role: 'tab', 'aria-selected': 'false', 'data-tab': 'employee' },
      'Employee',
    ),
  );

  const offlineNote = navigator.onLine
    ? null
    : h(
        'p',
        { class: 'banner banner-warn' },
        'You appear to be offline. Signing in for the first time needs a connection to the server.',
      );

  const screen = h(
    'div',
    { class: 'login-screen' },
    h(
      'div',
      { class: 'login-brand' },
      h('img', { src: '/icons/icon-192.png', alt: '', width: '64', height: '64' }),
      h('div', {}, h('p', { class: 'brand-kicker' }, 'Daybook'), h('p', { class: 'muted' }, 'In-house daily earnings')),
    ),
    offlineNote,
    tabs,
    error,
    adminForm,
    employeeForm,
    h('p', { class: 'login-foot' }, 'Server-side session · data stays on your network'),
  );

  function switchTab(tab) {
    const admin = tab === 'admin';
    adminForm.hidden = !admin;
    employeeForm.hidden = admin;
    for (const button of tabs.querySelectorAll('.login-tab')) {
      const active = button.dataset.tab === tab;
      button.classList.toggle('login-tab-active', active);
      button.setAttribute('aria-selected', String(active));
    }
    error.textContent = '';
  }

  tabs.addEventListener(
    'click',
    (event) => {
      const button = event.target.closest('[data-tab]');
      if (button) switchTab(button.dataset.tab);
    },
    { signal: abort.signal },
  );

  async function submit(form, action) {
    const submitButton = form.querySelector('button[type="submit"]');
    submitButton.disabled = true;
    const original = submitButton.textContent;
    submitButton.replaceChildren(spinner('Signing in…'));
    try {
      const identity = await action();
      await ctx.onAuthenticated(identity);
    } catch (requestError) {
      if (requestError?.status === 401) error.textContent = 'Wrong credentials';
      else if (requestError?.status === 429) {
        error.textContent = 'Too many attempts. Wait a few minutes and try again.';
      } else if (requestError?.status === 0) {
        error.textContent = 'Cannot reach the server. Check Wi-Fi and the server address.';
      } else if (requestError?.fields?.length) {
        error.textContent = requestError.fields.map((field) => field.message).join(' ');
      } else {
        error.textContent = requestError?.message ?? 'Sign-in failed';
      }
    } finally {
      submitButton.disabled = false;
      submitButton.replaceChildren(original);
    }
  }

  adminForm.addEventListener(
    'submit',
    (event) => {
      event.preventDefault();
      error.textContent = '';
      if (!adminPassword.value) {
        error.textContent = 'Enter the admin password';
        return;
      }
      void submit(adminForm, () => ctx.api.login(adminPassword.value));
    },
    { signal: abort.signal },
  );

  employeeForm.addEventListener(
    'submit',
    (event) => {
      event.preventDefault();
      error.textContent = '';
      const code = employeeCode.value.trim();
      const pin = employeePin.value;
      if (!code) {
        error.textContent = 'Enter your employee code';
        return;
      }
      if (!/^\d{4}$/.test(pin)) {
        error.textContent = 'Enter your 4-digit PIN';
        return;
      }
      void submit(employeeForm, () => ctx.api.loginEmployee(code, pin));
    },
    { signal: abort.signal },
  );

  root.replaceChildren(screen);
  adminPassword.focus();

  return () => abort.abort();
}
