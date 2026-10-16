// Noobietoria — worlds page: featured carousel + filterable worlds grid.
document.addEventListener("DOMContentLoaded", async () => {
    if (!window.API || !window.API.requireAuth()) return;

    const carousel = document.querySelector(".hero-carousel");
    const grid = document.querySelector(".worlds-grid");
    const searchInput = document.querySelector(".toolbar-search input");
    const selects = document.querySelectorAll(".worlds-toolbar .select");
    if (!grid) return;

    const thumbFallbacks = [
        "linear-gradient(135deg,#3aa6ff,#1f8fef)",
        "linear-gradient(135deg,#a06bfa,#6d28d9)",
        "linear-gradient(135deg,#2fd06e,#1fae57)",
        "linear-gradient(135deg,#ff9636,#e26a00)",
        "linear-gradient(135deg,#ff5c5c,#c22e2e)",
        "linear-gradient(135deg,#ffc93c,#c79a1a)",
    ];

    const worldCard = (w, i) => `
        <article class="world-card" data-world="${w.id}" style="cursor:pointer;">
            <div class="thumb" style="background:${w.thumbColor || thumbFallbacks[i % thumbFallbacks.length]};">${w.emoji}</div>
            <h3 class="world-card__title">${window.API.escapeHtml(w.name)}</h3>
            <div class="world-card__stats"><span>👤 ${w.playing}</span><span class="like">👍 ${w.likesPct}%</span></div>
        </article>`;

    /* ----- featured carousel ----- */
    if (carousel) {
        try {
            const data = await window.API.gql(`query { featuredWorlds { id name description creator { username } thumbColor } }`);
            const featured = data.featuredWorlds;
            if (featured.length) {
                const dots = carousel.querySelector(".hero-dots");
                carousel.querySelectorAll(".hero-slide").forEach((s) => s.remove());
                if (dots) dots.innerHTML = "";
                featured.forEach((w, i) => {
                    const slide = document.createElement("div");
                    slide.className = "hero-slide" + (i === 0 ? " is-active" : "");
                    slide.innerHTML = `
                        <div class="hero-slide__bg" ${w.thumbColor ? `style="background:${w.thumbColor};"` : ""}></div>
                        <div class="hero-slide__content">
                            <h1>${window.API.escapeHtml(w.name)}</h1>
                            <p class="byline">By ${window.API.escapeHtml(w.creator ? w.creator.username : "unknown")}</p>
                            <p class="desc">${window.API.escapeHtml(w.description)}</p>
                        </div>`;
                    carousel.insertBefore(slide, carousel.querySelector(".hero-arrow"));
                    if (dots) {
                        const dot = document.createElement("button");
                        if (i === 0) dot.className = "is-active";
                        dot.setAttribute("aria-label", `Slide ${i + 1}`);
                        dot.addEventListener("click", () => restartCarousel());
                        dots.appendChild(dot);
                    }
                });
            }
        } catch (err) { /* keep the static slides as fallback */ }
    }

    function restartCarousel() {
        const next = carousel.querySelector(".hero-arrow--next");
        if (next) next.click();
    }

    /* ----- join dialog: server list straight from the website API ----- */
    function openJoinDialog(placeId, placeName) {
        if (!window.NooServers) {
            window.API.toast("Server directory unavailable", "error");
            return;
        }
        window.NooServers.openJoinDialog(placeId, placeName, (server) => {
            // The player picked a server; the desktop client takes it from here.
            const deepLink = `noobietoria://join/${server.server_id}`;
            if (navigator.clipboard) {
                navigator.clipboard.writeText(deepLink).catch(() => {});
            }
            window.API.toast(`Joining ${server.server_id} (${server.host}:${server.port}) 🎮`);
        });
    }

    /* ----- worlds grid ----- */
    async function loadWorlds() {
        const genre = selects[0] ? selects[0].value : "";
        const version = selects[1] ? selects[1].value : "";
        const sortMap = { "Trending": "TRENDING", "Top Rated": "TOP_RATED", "Most Players": "MOST_PLAYERS", "Newest": "NEWEST" };
        const sort = selects[2] ? sortMap[selects[2].value] || "TRENDING" : "TRENDING";

        try {
            const data = await window.API.gql(`query($s: String, $g: String, $v: String, $sort: WorldSort) {
                worlds(search: $s, genre: $g, version: $v, sort: $sort) {
                    id name playing likesPct thumbColor emoji
                }
            }`, { s: searchInput ? searchInput.value : "", g: genre, v: version, sort });

            grid.innerHTML = data.worlds.length
                ? data.worlds.map(worldCard).join("")
                : '<p class="section-eyebrow">No worlds match your filters.</p>';

            grid.querySelectorAll("[data-world]").forEach((card) => {
                card.addEventListener("click", () => {
                    const placeId = card.dataset.world;
                    const title = card.querySelector(".world-card__title");
                    const placeName = title ? title.textContent.trim() : placeId;
                    openJoinDialog(placeId, placeName);
                });
            });
        } catch (err) {
            window.API.toast("Could not load worlds: " + err.message, "error");
        }
    }

    let timer = null;
    if (searchInput) {
        searchInput.addEventListener("input", () => {
            clearTimeout(timer);
            timer = setTimeout(loadWorlds, 300);
        });
    }
    selects.forEach((sel) => sel.addEventListener("change", loadWorlds));

    loadWorlds();
});
