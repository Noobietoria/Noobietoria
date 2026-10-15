// Noobietoria — guilds page: trending list, filters, search, create/join/leave.
document.addEventListener("DOMContentLoaded", async () => {
    if (!window.API || !window.API.requireAuth()) return;

    const filterList = document.querySelector(".guilds-side .filter-list");
    const yourGuilds = document.querySelector(".your-guilds");
    const grid = document.querySelector(".guilds-grid");
    const createBtn = document.querySelector(".guilds-main__head .btn--blue");
    const searchInput = document.querySelector(".guilds-main__search input");
    if (!grid) return;

    let filter = "TRENDING";
    const FILTER_KEYS = ["TRENDING", "TOP", "NEW", "YOURS"];

    const bannerStyle = (guild) => {
        const c = guild.bannerColors && guild.bannerColors[0];
        return c ? `style="background:${c};"` : 'style="background:linear-gradient(135deg,#6d28d9,#4c1d95);"';
    };

    const guildCard = (guild) => `
        <article class="guild-card" data-guild="${guild.id}">
            <div class="guild-card__banner" ${bannerStyle(guild)}>
                <div class="thumb" style="background:rgba(0,0,0,.35);color:#fff;">${guild.emoji}</div>
            </div>
            <div class="guild-card__body">
                <h3 class="guild-card__name">${window.API.escapeHtml(guild.name)} ${guild.verified ? '<span class="verified">✔</span>' : ""}</h3>
                <p class="guild-card__desc">${window.API.escapeHtml(guild.description)}</p>
                <div class="guild-card__members">👤 ${window.API.formatCount(guild.memberCount)}</div>
                <div style="margin-top:10px;">
                    ${guild.joined
                        ? `<button class="btn btn--outline btn--sm" data-leave="${guild.id}">Leave</button>`
                        : `<button class="btn btn--blue btn--sm" data-join="${guild.id}">＋ Join</button>`}
                </div>
            </div>
        </article>`;

    function bindCardButtons() {
        grid.querySelectorAll("[data-join]").forEach((btn) => {
            btn.addEventListener("click", async () => {
                btn.disabled = true;
                try {
                    await window.API.gql(`mutation($id: ID!){ joinGuild(guildId: $id) { id } }`, { id: btn.dataset.join });
                    window.API.toast("Joined the guild!");
                    loadAll();
                } catch (err) { window.API.toast(err.message, "error"); btn.disabled = false; }
            });
        });
        grid.querySelectorAll("[data-leave]").forEach((btn) => {
            btn.addEventListener("click", async () => {
                btn.disabled = true;
                try {
                    await window.API.gql(`mutation($id: ID!){ leaveGuild(guildId: $id) { id } }`, { id: btn.dataset.leave });
                    window.API.toast("You left the guild");
                    loadAll();
                } catch (err) { window.API.toast(err.message, "error"); btn.disabled = false; }
            });
        });
    }

    async function loadGuilds() {
        const data = await window.API.gql(`query($f: GuildFilter, $s: String) {
            guilds(filter: $f, search: $s) { id name description emoji bannerColors verified memberCount joined }
        }`, { f: filter, s: searchInput ? searchInput.value : "" });

        grid.innerHTML = data.guilds.length
            ? data.guilds.map(guildCard).join("")
            : '<p class="section-eyebrow">No guilds found.</p>';
        bindCardButtons();
    }

    async function loadYourGuilds() {
        if (!yourGuilds) return;
        const data = await window.API.gql(`query { myGuilds { id name memberCount bannerColors emoji } }`);
        yourGuilds.innerHTML = data.myGuilds.length ? data.myGuilds.map((g) => `
            <div class="guild-mini">
                <div class="guild-mini__logo" ${bannerStyle(g)}>${g.emoji || ""}</div>
                <div>
                    <div class="guild-mini__name">${window.API.escapeHtml(g.name)}</div>
                    <div class="guild-mini__members">👤 ${window.API.formatCount(g.memberCount)}</div>
                </div>
            </div>`).join("")
            : '<p class="section-eyebrow">You have not joined any guilds yet.</p>';
    }

    function loadAll() {
        loadGuilds();
        loadYourGuilds();
    }

    // filters
    if (filterList) {
        filterList.querySelectorAll(".filter-item").forEach((item, i) => {
            item.addEventListener("click", (e) => {
                e.preventDefault();
                filterList.querySelectorAll(".filter-item").forEach((el) => el.classList.remove("is-active"));
                item.classList.add("is-active");
                filter = FILTER_KEYS[i] || "TRENDING";
                loadGuilds();
            });
        });
    }

    // search
    let timer = null;
    if (searchInput) {
        searchInput.addEventListener("input", () => {
            clearTimeout(timer);
            timer = setTimeout(loadGuilds, 300);
        });
    }

    // create guild modal
    if (createBtn) {
        createBtn.addEventListener("click", () => {
            window.NooModal.open({
                title: "Create a Guild",
                body: `
                    <label class="modal__label">Guild name</label>
                    <input class="modal__input" id="guild-name" maxlength="60" placeholder="e.g. Noobietoria Builders">
                    <label class="modal__label">Description</label>
                    <textarea class="modal__input" id="guild-desc" rows="3" maxlength="300" placeholder="What is your guild about?"></textarea>`,
                submitLabel: "Create Guild",
                onSubmit: async () => {
                    const name = document.getElementById("guild-name").value.trim();
                    const description = document.getElementById("guild-desc").value.trim();
                    if (name.length < 3) throw new Error("Guild name must be at least 3 characters.");
                    await window.API.gql(`mutation($n: String!, $d: String){ createGuild(name: $n, description: $d) { id } }`, { n: name, d: description });
                    window.API.toast(`Guild "${name}" created!`);
                    loadAll();
                },
            });
        });
    }

    loadAll();
});
