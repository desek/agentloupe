// @agents-index The scheme script: a classic script in the head that sets the color scheme on the root element before the first paint.
// It runs before the stylesheets apply and before any module, so it cannot import scheme.resolve.js; it applies the
// same rule, and web/test/scheme.resolve.test.js checks that both give the same scheme for each row of the rule.
(function (doc, win) {
  var stored = null;
  try { stored = win.localStorage.getItem('dashboard.scheme'); } catch (e) { stored = null; }
  var light = false;
  try { light = !!(win.matchMedia && win.matchMedia('(prefers-color-scheme: light)').matches); } catch (e) { light = false; }
  var state = stored === 'light' || stored === 'dark' ? stored : 'system';
  doc.documentElement.setAttribute('data-scheme', state === 'system' ? (light ? 'light' : 'dark') : state);
}(document, window));
