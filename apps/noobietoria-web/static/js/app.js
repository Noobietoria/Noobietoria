// Noobietoria — internal app interactions
document.addEventListener("DOMContentLoaded", () => {
    // ----- active nav highlighting via <body data-page="...">
    const page = document.body.dataset.page;
    if (page) {
        document.querySelectorAll("[data-nav]").forEach((link) => {
            if (link.dataset.nav === page) {
                link.classList.add("is-active");
            }
        });
    }

    // ----- hero carousel (worlds page)
    const carousel = document.querySelector(".hero-carousel");
    if (carousel) {
        const slides = carousel.querySelectorAll(".hero-slide");
        const dots = carousel.querySelectorAll(".hero-dots button");
        let current = 0;
        let timer = null;

        const show = (index) => {
            current = (index + slides.length) % slides.length;
            slides.forEach((s, i) => s.classList.toggle("is-active", i === current));
            dots.forEach((d, i) => d.classList.toggle("is-active", i === current));
        };

        const restart = () => {
            clearInterval(timer);
            timer = setInterval(() => show(current + 1), 6000);
        };

        carousel.querySelector(".hero-arrow--prev")?.addEventListener("click", () => { show(current - 1); restart(); });
        carousel.querySelector(".hero-arrow--next")?.addEventListener("click", () => { show(current + 1); restart(); });
        dots.forEach((d, i) => d.addEventListener("click", () => { show(i); restart(); }));

        restart();
    }

    // ----- simple tab groups (people / create / character)
    document.querySelectorAll("[data-tabs]").forEach((group) => {
        const tabs = group.querySelectorAll("[data-tab]");
        tabs.forEach((tab) => {
            tab.addEventListener("click", () => {
                tabs.forEach((t) => t.classList.remove("is-active"));
                tab.classList.add("is-active");
                const scope = group.dataset.tabs;
                document.querySelectorAll(`[data-tab-panel="${scope}"]`).forEach((panel) => {
                    panel.style.display = panel.dataset.panelId === tab.dataset.tab ? "" : "none";
                });
            });
        });
    });

    // ----- avatar body color swatches (character page)
    const preview = document.querySelector(".avatar-preview");
    document.querySelectorAll(".color-swatch").forEach((swatch) => {
        swatch.addEventListener("click", () => {
            document.querySelectorAll(".color-swatch").forEach((s) => s.classList.remove("is-active"));
            swatch.classList.add("is-active");
            if (preview) preview.style.filter = `hue-rotate(${swatch.dataset.hue || 0}deg)`;
        });
    });
});