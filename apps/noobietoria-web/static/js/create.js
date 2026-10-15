// Noobietoria — create page: my worlds (create/delete) + community creations.
document.addEventListener("DOMContentLoaded", async () => {
    if (!window.API || !window.API.requireAuth()) return;

    const minePanel = document.querySelector('[data-tab-panel="create"][data-panel-id="mine"]');
    const communityPanel = document.querySelector('[data-tab-panel="create"][data-panel-id="community"]');
    const createWorldBtn = document.querySelector(".create-world-box .btn--blue");
    if (!minePanel) return;

    const thumbFallbacks = [
        "linear-gradient(135deg,#3aa6ff,#1f8fef)",
        "linear-gradient(135deg,#a06bfa,#6d28d9)",
        "linear-gradient(135deg,#2fd06e,#1fae57)",
        "linear-gradient(135deg,#ff9636,#e26a00)",
    ];

    const worldRow = (w) => `
        <div class="world-row card" data-world-row="${w.id}">
            <div class="thumb" style="background:${w.thumbColor || thumbFallbacks[w.id % thumbFallbacks.length]};">${w.emoji}</div>
            <div class="world-row__info">
                <h3 class="world-row__name">${window.API.escapeHtml(w.name)}</h3>
                <div class="world-row__stats">
                    <span>${w.visibility === "private" ? "🔒 Private" : "🌐 Public"}</span>
                    <span>▶ ${w.playing}</span>
                    <span>👥 ${window.API.escapeHtml(w.genre)}</span>
                    <span>👍 ${w.likesPct}%</span>
                </div>
            </div>
            <div class="world-row__actions">
                <button class="btn btn--outline btn--sm" data-delete-world="${w.id}">🗑️ Delete</button>
            </div>
        </div>`;

    function renderMine(worlds) {
        const box = minePanel.querySelector(".create-world-box");
        const rows = worlds.map(worldRow).join("") ||
            '<p class="section-eyebrow">You have no worlds yet — create your first one above!</p>';
        minePanel.querySelectorAll(".world-row").forEach((el) => el.remove());
        box.insertAdjacentHTML("afterend", rows);

        minePanel.querySelectorAll("[data-delete-world]").forEach((btn) => {
            btn.addEventListener("click", async () => {
                if (!window.confirm("Delete this world? This cannot be undone.")) return;
                btn.disabled = true;
                try {
                    await window.API.gql(`mutation($id: ID!){ deleteWorld(id: $id) }`, { id: btn.dataset.deleteWorld });
                    window.API.toast("World deleted");
                    loadMine();
                } catch (err) {
                    window.API.toast(err.message, "error");
                    btn.disabled = false;
                }
            });
        });
    }

    async function loadMine() {
        const data = await window.API.gql(`query { myWorlds { id name genre visibility playing likesPct thumbColor emoji } }`);
        renderMine(data.myWorlds);
    }

    async function loadCommunity() {
        const data = await window.API.gql(`query { worlds(sort: NEWEST) { id name creator { username } genre playing likesPct thumbColor emoji } }`);
        communityPanel.innerHTML = data.worlds.length
            ? data.worlds.map((w) => `
                <div class="world-row card">
                    <div class="thumb" style="background:${w.thumbColor || thumbFallbacks[w.id % thumbFallbacks.length]};">${w.emoji}</div>
                    <div class="world-row__info">
                        <h3 class="world-row__name">${window.API.escapeHtml(w.name)}</h3>
                        <div class="world-row__stats">
                            <span>by ${window.API.escapeHtml(w.creator ? w.creator.username : "unknown")}</span>
                            <span>${window.API.escapeHtml(w.genre)}</span>
                            <span>▶ ${w.playing}</span>
                            <span>👍 ${w.likesPct}%</span>
                        </div>
                    </div>
                </div>`).join("")
            : '<p class="section-eyebrow">No community creations yet.</p>';
    }

    if (createWorldBtn) {
        createWorldBtn.addEventListener("click", () => {
            window.NooModal.open({
                title: "Create World",
                body: `
                    <label class="modal__label">World name</label>
                    <input class="modal__input" id="world-name" maxlength="120" placeholder="e.g. My Awesome World">
                    <label class="modal__label">Description</label>
                    <textarea class="modal__input" id="world-desc" rows="3" maxlength="500" placeholder="What's inside?"></textarea>
                    <label class="modal__label">Genre</label>
                    <select class="modal__input" id="world-genre">
                        <option>Hangout</option><option>Adventure</option>
                        <option>Obby</option><option>PvP</option>
                    </select>
                    <label class="modal__label">Visibility</label>
                    <select class="modal__input" id="world-vis">
                        <option value="public">Public</option>
                        <option value="private">Private</option>
                    </select>`,
                submitLabel: "Create World",
                onSubmit: async () => {
                    const name = document.getElementById("world-name").value.trim();
                    const description = document.getElementById("world-desc").value.trim();
                    const genre = document.getElementById("world-genre").value;
                    const visibility = document.getElementById("world-vis").value;
                    if (name.length < 3) throw new Error("World name must be at least 3 characters.");
                    await window.API.gql(
                        `mutation($n: String!, $d: String, $g: String, $v: String){ createWorld(name: $n, description: $d, genre: $g, visibility: $v) { id } }`,
                        { n: name, d: description, g: genre, v: visibility },
                    );
                    window.API.toast(`World "${name}" created!`);
                    loadMine();
                },
            });
        });
    }

    loadMine();
    loadCommunity();
});
