// Noobietoria — landing page animations (GSAP + ScrollTrigger)
document.addEventListener("DOMContentLoaded", () => {
    if (typeof gsap === "undefined") return;
    if (typeof ScrollTrigger !== "undefined") gsap.registerPlugin(ScrollTrigger);

    /* ---------- 1. Intro timeline ---------- */
    const intro = gsap.timeline({ defaults: { ease: "power3.out" } });

    intro
        .from(".navbar", { y: -50, opacity: 0, duration: 0.6 })
        .from(".eyebrow", { y: 18, opacity: 0, scale: 0.9, duration: 0.5 }, "-=0.25")
        .from(".hero__title .word", {
            y: 40, opacity: 0, rotateX: -60, duration: 0.7, stagger: 0.12,
            transformOrigin: "50% 100%"
        }, "-=0.3")
        .from(".hero__subtitle", { y: 18, opacity: 0, duration: 0.5 }, "-=0.35")
        .from(".hero__actions .btn", { y: 18, opacity: 0, duration: 0.45, stagger: 0.1 }, "-=0.3")
        .from(".hero__stats .stat", { y: 18, opacity: 0, duration: 0.45, stagger: 0.09 }, "-=0.25");

    /* ---------- 2. Floating blobs (infinite drift + mouse parallax) ---------- */
    gsap.to(".hero__blob--one", {
        x: 60, y: 40, scale: 1.15, duration: 6,
        yoyo: true, repeat: -1, ease: "sine.inOut"
    });
    gsap.to(".hero__blob--two", {
        x: -50, y: -35, scale: 0.9, duration: 7,
        yoyo: true, repeat: -1, ease: "sine.inOut", delay: 0.8
    });

    // Gentle mouse parallax on the blobs
    const blobs = document.querySelectorAll(".hero__blob");
    window.addEventListener("mousemove", (e) => {
        const cx = window.innerWidth / 2;
        const cy = window.innerHeight / 2;
        blobs.forEach((blob, i) => {
            const depth = i === 0 ? 26 : 16;
            gsap.to(blob, {
                x: ((e.clientX - cx) / cx) * depth,
                y: ((e.clientY - cy) / cy) * depth,
                duration: 1.1,
                ease: "power2.out"
            });
        });
    });

    /* ---------- 3. Scroll-triggered reveals ---------- */
    if (typeof ScrollTrigger !== "undefined") {
        // Feature cards stagger in
        gsap.from(".features__grid .card", {
            scrollTrigger: { trigger: ".features", start: "top 75%" },
            y: 48, opacity: 0, rotate: -2, duration: 0.6, stagger: 0.12,
            ease: "back.out(1.6)"
        });

        // Section titles slide in
        gsap.from(".features .section-title", {
            scrollTrigger: { trigger: ".features", start: "top 80%" },
            y: 30, opacity: 0, duration: 0.6
        });

        // Press kit blocks pop in
        gsap.from(".presskit__asset", {
            scrollTrigger: { trigger: ".presskit", start: "top 75%" },
            y: 40, opacity: 0, scale: 0.92, duration: 0.55, stagger: 0.12,
            ease: "back.out(1.7)"
        });
        gsap.from(".presskit__row > *", {
            scrollTrigger: { trigger: ".presskit__row", start: "top 85%" },
            y: 30, opacity: 0, duration: 0.55, stagger: 0.15
        });

        // CTA box zoom
        gsap.from(".cta", {
            scrollTrigger: { trigger: ".cta", start: "top 85%" },
            scale: 0.9, opacity: 0, duration: 0.7, ease: "back.out(1.4)"
        });

        // Footer fade
        gsap.from(".footer", {
            scrollTrigger: { trigger: ".footer", start: "top 95%" },
            opacity: 0, duration: 0.8
        });
    }

    /* ---------- 4. Animated stat counters ---------- */
    document.querySelectorAll(".stat__num[data-count]").forEach((el) => {
        const target = parseInt(el.dataset.count, 10);
        const suffix = el.dataset.suffix || "";
        const counter = { value: 0 };

        const render = () => {
            el.textContent = (target >= 1000
                ? Math.round(counter.value / 100) / 10 + "K"
                : Math.round(counter.value)) + suffix;
        };

        if (typeof ScrollTrigger !== "undefined") {
            gsap.to(counter, {
                value: target, duration: 1.8, ease: "power2.out",
                scrollTrigger: { trigger: el, start: "top 90%" },
                onUpdate: render
            });
        } else {
            counter.value = target;
            render();
        }
    });

    /* ---------- 5. Magnetic buttons ---------- */
    document.querySelectorAll(".btn--primary, .btn--small").forEach((btn) => {
        btn.addEventListener("mousemove", (e) => {
            const rect = btn.getBoundingClientRect();
            gsap.to(btn, {
                x: (e.clientX - rect.left - rect.width / 2) * 0.25,
                y: (e.clientY - rect.top - rect.height / 2) * 0.35,
                duration: 0.3
            });
        });
        btn.addEventListener("mouseleave", () => {
            gsap.to(btn, { x: 0, y: 0, duration: 0.45, ease: "elastic.out(1, 0.5)" });
        });
    });

    /* ---------- 6. Navbar link hover wiggle ---------- */
    document.querySelectorAll(".navbar__links a:not(.btn)").forEach((link) => {
        link.addEventListener("mouseenter", () => {
            gsap.fromTo(link, { rotate: -3 }, { rotate: 0, duration: 0.5, ease: "elastic.out(1, 0.4)" });
        });
    });
});