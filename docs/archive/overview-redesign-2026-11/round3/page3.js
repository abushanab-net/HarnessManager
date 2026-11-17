// Round three on top of round two's page code: labels are quiet (names only) unless they are on the selected route or
// carry a warning, and the feedback form has two maps.
(function () {
  H2.dimmer = (map, bd) => (lit) => {
    map.querySelectorAll('[data-node]').forEach((g) => g.classList.toggle('dim', !!lit && !lit.has(g.dataset.node)));
    map.querySelectorAll('.lb[data-for]').forEach((d) => { const on = !!lit && lit.has(d.dataset.for); d.classList.toggle('dim', !!lit && !on); d.classList.toggle('lit', on); });
    map.querySelectorAll('path.e').forEach((p) => p.classList.toggle('dim', !!lit && !(lit.has(p.dataset.a) && lit.has(p.dataset.b))));
    if (bd) bd.filter(lit ? (e) => lit.has(e.a) && lit.has(e.b) : null);
  };
})();
