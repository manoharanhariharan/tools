// Sign-in screen. Sessions are server-side cookies; the PWA holds no tokens.

import { h, spinner } from '../dom.js';

export function renderLogin(root, ctx) {
  const abort = new AbortController();
  const error = h('p', { class: 'form-error', role: 'alert' });
  const password = h('input', {
    type: 'password',
    name: 'password',
    autocomplete: 'current-password',
    required: true,
    minlength: '10',
    'aria-describedby': 'login-error',
  });
  error.id = 'login-error';
  const submit = h('button', { class: 'btn btn-primary btn-block', type: 'submit' }, 'Sign in');
  const form = h(
    'form',
    { class: 'login-card', novalidate: true },
    h('h1', {}, 'Sign in'),
    h('p', { class: 'muted' }, 'Use the admin password for this Daybook server.'),
    h('div', { class: 'field' }, h('label', {}, h('span', { class: 'field-label' }, 'Password'), password)),
    error,
    submit,
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
    form,
    h('p', { class: 'login-foot' }, 'Server-side session · data stays on your network'),
  );

  root.replaceChildren(screen);
  password.focus();

  form.addEventListener(
    'submit',
    async (event) => {
      event.preventDefault();
      error.textContent = '';
      const value = password.value;
      if (!value) {
        error.textContent = 'Enter the admin password';
        return;
      }
      submit.disabled = true;
      submit.replaceChildren(spinner('Signing in…'));
      try {
        await ctx.api.login(value);
        password.value = '';
        await ctx.onAuthenticated();
      } catch (requestError) {
        if (requestError?.status === 401) error.textContent = 'Wrong password';
        else if (requestError?.status === 429) {
          error.textContent = 'Too many attempts. Wait a few minutes and try again.';
        } else if (requestError?.status === 0) {
          error.textContent = 'Cannot reach the server. Check Wi-Fi and the server address.';
        } else {
          error.textContent = requestError?.message ?? 'Sign-in failed';
        }
        password.select();
      } finally {
        submit.disabled = false;
        submit.replaceChildren('Sign in');
      }
    },
    { signal: abort.signal },
  );

  return () => abort.abort();
}
