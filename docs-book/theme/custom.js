/* mneme docs — motion layer
 *
 * Pure vanilla JS. No jQuery, no Webflow runtime.
 * Adds:
 *   - IntersectionObserver fade-up on scroll
 *   - Stat count-up when stats enter viewport
 *   - Terminal typing animation cascade
 *   - Stagger entrance on feature/install cards
 *
 * Respects prefers-reduced-motion: skips animation, shows final state.
 */

(function () {
  'use strict';

  if (typeof window === 'undefined') return;

  const reduce = window.matchMedia &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  /* ----- Helpers --------------------------------------------------------- */

  function ready(fn) {
    if (document.readyState !== 'loading') fn();
    else document.addEventListener('DOMContentLoaded', fn);
  }

  /* ----- Scroll-triggered reveal (.mneme-reveal) ------------------------- */
  /* Adds .in-view when element scrolls into the viewport. CSS handles the
   * actual fade-up transform. Uses IntersectionObserver. */

  function setupScrollReveal() {
    const targets = document.querySelectorAll(
      '.mneme-feature, .mneme-install-card, .mneme-read-card, ' +
      '.mneme-faq-item, .mneme-trust-item, .mneme-section-heading, ' +
      '.mneme-final-cta, .mneme-callout'
    );

    if (reduce) {
      targets.forEach(t => t.classList.add('in-view'));
      return;
    }

    if (!('IntersectionObserver' in window)) {
      targets.forEach(t => t.classList.add('in-view'));
      return;
    }

    const io = new IntersectionObserver((entries) => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          entry.target.classList.add('in-view');
          io.unobserve(entry.target);
        }
      });
    }, {
      rootMargin: '0px 0px -10% 0px',
      threshold: 0.05
    });

    targets.forEach((el, i) => {
      el.style.setProperty('--mneme-stagger', `${(i % 6) * 80}ms`);
      io.observe(el);
    });
  }

  /* ----- Count-up animation on stat numbers ------------------------------ */
  /* When .mneme-stats or .mneme-trust-strip enters viewport, animate the
   * .num spans from 0 → final value over ~1200ms with ease-out cubic. */

  function parseNum(text) {
    const m = String(text).match(/^([0-9.]+)\s*(.*)$/);
    if (!m) return null;
    return { value: parseFloat(m[1]), suffix: m[2] || '', original: text };
  }

  function easeOutCubic(t) {
    return 1 - Math.pow(1 - t, 3);
  }

  function animateNum(el, target, suffix, duration) {
    const start = performance.now();
    const isInt = Number.isInteger(target);
    const fmt = (v) => {
      if (isInt) return Math.round(v).toString();
      return v.toFixed(target % 1 === 0 ? 0 : 1);
    };

    function step(now) {
      const elapsed = now - start;
      const t = Math.min(elapsed / duration, 1);
      const eased = easeOutCubic(t);
      el.textContent = fmt(target * eased) + suffix;
      if (t < 1) requestAnimationFrame(step);
      else el.textContent = fmt(target) + suffix;
    }

    requestAnimationFrame(step);
  }

  function setupCountUp() {
    const groups = document.querySelectorAll(
      '.mneme-stats, .mneme-trust-strip'
    );
    if (!groups.length) return;

    if (reduce || !('IntersectionObserver' in window)) return;

    const io = new IntersectionObserver((entries) => {
      entries.forEach(entry => {
        if (!entry.isIntersecting) return;
        const root = entry.target;
        const nums = root.querySelectorAll('.num, .trust-num');

        nums.forEach((el, i) => {
          if (el.dataset.counted) return;

          const original = el.textContent.trim();

          // Special-cases that aren't simple numbers
          if (original.includes('→') || original.includes('->')) {
            // "2/10 → 6/10" — leave as is, just fade
            el.dataset.counted = '1';
            return;
          }
          if (/[a-zA-Z]/.test(original) && !original.endsWith('%')) {
            // "<500 ms", "0 bytes" — animate the leading number only
            const m = original.match(/^([<>]?\s*)([0-9.]+)\s*(.*)$/);
            if (m) {
              const prefix = m[1] || '';
              const target = parseFloat(m[2]);
              const suffix = m[3] || '';
              el.dataset.counted = '1';
              setTimeout(() => {
                el.textContent = prefix + '0 ' + suffix;
                animateNum({
                  textContent: el.textContent,
                  set textContent(v) { el.textContent = prefix + v; }
                }, target, ' ' + suffix, 1200);
              }, i * 80);
            }
            return;
          }

          const parsed = parseNum(original);
          if (!parsed) return;

          el.dataset.counted = '1';
          setTimeout(() => {
            el.textContent = '0' + parsed.suffix;
            animateNum(el, parsed.value, parsed.suffix, 1200);
          }, i * 80);
        });

        io.unobserve(root);
      });
    }, { threshold: 0.4 });

    groups.forEach(g => io.observe(g));
  }

  /* ----- Hero entrance stagger ------------------------------------------ */
  /* Just adds .hero-loaded class to .mneme-hero on load. CSS does the work. */

  function setupHeroEntrance() {
    const hero = document.querySelector('.mneme-hero');
    if (!hero) return;
    if (reduce) {
      hero.classList.add('hero-loaded');
      return;
    }
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        hero.classList.add('hero-loaded');
      });
    });
  }

  /* ----- Boot ------------------------------------------------------------ */

  ready(function () {
    setupHeroEntrance();
    setupScrollReveal();
    setupCountUp();
  });
})();
