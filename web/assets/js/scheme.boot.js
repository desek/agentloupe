// @agents-index The scheme script: a classic script in the head that sets the color scheme before the first paint.
// It runs before any module, so it repeats the rule of scheme.resolve.js; scheme.resolve.test.js checks both.
(function (d, w) {
  var s = null, l = false;
  try { s = w.localStorage.getItem('dashboard.scheme'); } catch (e) {}
  try { l = w.matchMedia('(prefers-color-scheme: light)').matches; } catch (e) {}
  d.documentElement.dataset.scheme = s === 'light' || s === 'dark' ? s : l ? 'light' : 'dark';
}(document, window));
