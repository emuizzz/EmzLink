(() => {
  const root = document.documentElement;
  const $ = (id) => document.getElementById(id);
  const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // ripple on press
  document.addEventListener('pointerdown', (e) => {
    if (reduce) return;
    const host = e.target.closest && e.target.closest('.btn,.mode-row,.seg button,.tg-link');
    if (!host || host.disabled) return;
    const r = host.getBoundingClientRect();
    const size = Math.max(r.width, r.height) * 2.2;
    const dot = document.createElement('span');
    dot.className = 'fx-ripple';
    dot.style.width = dot.style.height = size + 'px';
    dot.style.left = e.clientX - r.left - size / 2 + 'px';
    dot.style.top = e.clientY - r.top - size / 2 + 'px';
    host.appendChild(dot);
    setTimeout(() => dot.remove(), 700);
  }, true);

  // page direction: moving down the menu slides content up, moving up slides it down
  const nav = $('nav');
  if (nav) {
    nav.addEventListener('click', (e) => {
      const item = e.target.closest('.nav-item');
      if (!item) return;
      const items = [...nav.querySelectorAll('.nav-item')];
      const from = items.findIndex((x) => x.classList.contains('on'));
      const to = items.indexOf(item);
      if (to === from || to < 0) return;
      root.style.setProperty('--vy', (to > from ? 12 : -12) + 'px');
    }, true);
  }

  // smooth colour change when the theme controls are used
  document.addEventListener('click', (e) => {
    if (reduce) return;
    if (e.target.closest('#segStyle button,#segMode button,.sw,#swatches button')) {
      root.classList.add('fx-theme');
      clearTimeout(root._fxT);
      root._fxT = setTimeout(() => root.classList.remove('fx-theme'), 600);
    }
  }, true);

  // status title swap animation
  const st = $('statusTitle');
  if (st && !reduce) {
    let t = null, last = st.dataset.t || st.textContent;
    new MutationObserver(() => {
      const now = st.dataset.t || st.textContent;
      if (now === last) return;
      last = now;
      clearTimeout(t);
      st.classList.remove('fx-swap'); void st.offsetWidth; st.classList.add('fx-swap');
      t = setTimeout(() => st.classList.remove('fx-swap'), 500);
    }).observe(st, { childList: true, characterData: true, subtree: true });
  }

  // mode list entrance only when the panel is opened
  const home = document.querySelector('.home');
  const list = $('modeList');
  if (home && list && !reduce) {
    let wasCollapsed = home.classList.contains('alts-collapsed');
    new MutationObserver(() => {
      const collapsed = home.classList.contains('alts-collapsed');
      if (wasCollapsed && !collapsed) {
        list.classList.add('fx-enter');
        setTimeout(() => list.classList.remove('fx-enter'), 900);
      }
      wasCollapsed = collapsed;
    }).observe(home, { attributes: true, attributeFilter: ['class'] });
  }
})();
