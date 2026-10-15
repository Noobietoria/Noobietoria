// Noobietoria — home page: friends strip + news feed (live from GraphQL).
document.addEventListener("DOMContentLoaded", async () => {
    if (!window.API || !window.API.requireAuth()) return;

    const strip = document.querySelector(".friends-strip");
    const newsGrid = document.querySelector(".news-grid");
    const welcomeName = document.querySelector(".welcome__name");
    const welcomeFigure = document.querySelector(".welcome__figure");

    try {
        const data = await window.API.gql(`query {
            me { username avatarEmoji bodyColorHue friendsCount }
            friends { id username status avatarEmoji }
            news { id title emoji thumbColor publishedAt }
        }`);

        if (welcomeName && data.me) welcomeName.textContent = data.me.username;
        if (welcomeFigure && data.me) {
            welcomeFigure.textContent = data.me.avatarEmoji;
            welcomeFigure.style.filter = `hue-rotate(${data.me.bodyColorHue || 0}deg)`;
        }

        if (strip) {
            if (!data.friends.length) {
                strip.innerHTML = '<p class="section-eyebrow">No friends yet — find some on the People page!</p>';
            } else {
                strip.innerHTML = data.friends.map((f) => `
                    <a class="friend-chip" href="profile.html?u=${encodeURIComponent(f.username)}">
                        <span class="av ${window.API.avatarClass(f.id)} is-online">${window.API.escapeHtml(f.avatarEmoji)}</span>
                        <span class="friend-chip__name">
                            <span class="friend-chip__status friend-chip__status--${f.status === "ingame" ? "ingame" : f.status === "online" ? "online" : "offline"}"></span>
                            ${window.API.escapeHtml(f.username)}
                        </span>
                    </a>`).join("");
            }
        }

        if (newsGrid) {
            newsGrid.innerHTML = data.news.map((n) => `
                <article class="news-card">
                    <div class="news-card__banner" ${n.thumbColor ? `style="background:${n.thumbColor};"` : ""}>${n.emoji}</div>
                    <div class="news-card__body">
                        <h3 class="news-card__title">${window.API.escapeHtml(n.title)}</h3>
                        <p class="news-card__time">${window.API.timeAgo(n.publishedAt)}</p>
                    </div>
                </article>`).join("");
        }
    } catch (err) {
        window.API.toast("Could not load home page: " + err.message, "error");
    }
});
