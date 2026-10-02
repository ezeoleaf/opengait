(() => {
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  const sections = document.querySelectorAll(".section, .hero-copy, .hero-stage");
  sections.forEach((el) => el.classList.add("reveal"));

  if (reduce) {
    sections.forEach((el) => el.classList.add("is-visible"));
    return;
  }

  const io = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("is-visible");
          io.unobserve(entry.target);
        }
      }
    },
    { threshold: 0.16, rootMargin: "0px 0px -40px 0px" },
  );

  sections.forEach((el) => io.observe(el));

  // Soft cadence tick on floating metric values (demo ambience only).
  const cadence = document.querySelector(".m1 strong");
  if (!cadence) return;
  let t = 0;
  const tick = () => {
    t += 0.04;
    const spm = 174 + Math.round(Math.sin(t) * 4 + Math.sin(t * 0.37) * 2);
    cadence.textContent = `${spm} SPM`;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
})();
