// Noobietoria — people page: friends list, incoming requests, global search.
document.addEventListener("DOMContentLoaded", async () => {
    if (!window.API || !window.API.requireAuth()) return;

    const friendsPanel = document.querySelector('[data-tab-panel="people"][data-panel-id="friends"]');
    const globalPanel = document.querySelector('[data-tab-panel="people"][data-panel-id="global"]');
    const searchInput = document.querySelector(".people-search input");
    const searchBtn = document.querySelector(".people-search button");
    const legend = document.querySelector(".people-legend");
    if (!friendsPanel || !globalPanel) return;

    let onlineCount = 0;
    let ingameCount = 0;

    const personRow = (user, opts = {}) => `
        <div class="person-row" data-user-row="${user.id}">
            <span class="av ${window.API.avatarClass(user.id)}">${window.API.escapeHtml(user.avatarEmoji)}</span>
            <div class="person-row__info">
                <div class="person-row__name"><span class="status-dot" ${window.API.statusDot(user.status)}></span>${window.API.escapeHtml(user.username)}</div>
                <p class="person-row__status">${window.API.escapeHtml(user.bio || "")}</p>
                <div class="person-row__meta">
                    <span>⭐ Level ${user.level}</span>
                    ${user.plusTier ? '<span class="plus">✦ ' + (user.plusTier === "plus_deluxe" ? "Plus Deluxe" : "Plus") + "</span>" : ""}
                </div>
            </div>
            <button class="btn ${opts.isFriend ? "btn--green" : "btn--blue"} btn--sm" data-friend-action="${user.id}" data-is-friend="${opts.isFriend ? "1" : "0"}">
                ${opts.isFriend ? "✔ Friends" : "＋ Add Friend"}
            </button>
            <a class="btn btn--ghost btn--sm" href="profile.html?u=${encodeURIComponent(user.username)}" title="View profile">👤</a>
        </div>`;

    const requestRow = (req) => `
        <div class="person-row" data-request-row="${req.id}">
            <span class="av ${window.API.avatarClass(req.from.id)}">${window.API.escapeHtml(req.from.avatarEmoji)}</span>
            <div class="person-row__info">
                <div class="person-row__name"><span class="status-dot" ${window.API.statusDot(req.from.status)}></span>${window.API.escapeHtml(req.from.username)}</div>
                <p class="person-row__status">wants to be your friend · ${window.API.timeAgo(req.createdAt)}</p>
            </div>
            <button class="btn btn--green btn--sm" data-accept-request="${req.id}">✔ Accept</button>
            <button class="btn btn--ghost btn--sm" data-decline-request="${req.id}">✕</button>
        </div>`;

    function bindFriendButtons(scope) {
        scope.querySelectorAll("[data-friend-action]").forEach((btn) => {
            btn.addEventListener("click", async () => {
                const isFriend = btn.dataset.isFriend === "1";
                btn.disabled = true;
                try {
                    if (isFriend) {
                        await window.API.gql(`mutation($id: ID!){ removeFriend(userId: $id) }`, { id: btn.dataset.friendAction });
                        btn.textContent = "＋ Add Friend";
                        btn.dataset.isFriend = "0";
                        btn.classList.replace("btn--green", "btn--blue");
                        window.API.toast("Friend removed");
                    } else {
                        await window.API.gql(`mutation($id: ID!){ sendFriendRequest(userId: $id) }`, { id: btn.dataset.friendAction });
                        btn.textContent = "Request sent ✓";
                        window.API.toast("Friend request sent");
                    }
                } catch (err) {
                    window.API.toast(err.message, "error");
                }
                btn.disabled = false;
            });
        });
    }

    function updateLegend(total) {
        if (!legend) return;
        legend.innerHTML = `
            <span><span class="legend-dot" style="background:var(--green);"></span>${onlineCount}</span>
            <span><span class="legend-dot" style="background:var(--blue);"></span>${ingameCount}</span>
            <span><span class="legend-dot" style="border:2px solid var(--muted);width:9px;height:9px;"></span>${total} friends</span>`;
    }

    async function loadFriends(search = "") {
        const data = await window.API.gql(`query {
            friends { id username bio avatarEmoji status level plusTier }
            friendRequests { id from { id username avatarEmoji status bio } createdAt }
        }`);
        onlineCount = data.friends.filter((f) => f.status === "online").length;
        ingameCount = data.friends.filter((f) => f.status === "ingame").length;
        updateLegend(data.friends.length);

        const q = search.trim().toLowerCase();
        const visible = data.friends.filter((f) => !q || f.username.toLowerCase().includes(q));

        const reqHtml = data.friendRequests.length
            ? `<h3 class="section-eyebrow" style="margin-bottom:10px;">Pending requests (${data.friendRequests.length})</h3>` +
              data.friendRequests.map(requestRow).join("")
            : "";
        const listHtml = visible.length
            ? visible.map((f) => personRow(f, { isFriend: true })).join("")
            : `<p class="section-eyebrow">${q ? "No friends match your search." : "No friends yet."}</p>`;

        friendsPanel.innerHTML = reqHtml + listHtml;

        friendsPanel.querySelectorAll("[data-accept-request]").forEach((btn) => {
            btn.addEventListener("click", async () => {
                btn.disabled = true;
                try {
                    await window.API.gql(`mutation($id: ID!){ acceptFriendRequest(requestId: $id) }`, { id: btn.dataset.acceptRequest });
                    window.API.toast("Friend added!");
                    loadFriends(searchInput ? searchInput.value : "");
                } catch (err) { window.API.toast(err.message, "error"); btn.disabled = false; }
            });
        });
        friendsPanel.querySelectorAll("[data-decline-request]").forEach((btn) => {
            btn.addEventListener("click", async () => {
                btn.disabled = true;
                try {
                    await window.API.gql(`mutation($id: ID!){ declineFriendRequest(requestId: $id) }`, { id: btn.dataset.declineRequest });
                    loadFriends(searchInput ? searchInput.value : "");
                } catch (err) { window.API.toast(err.message, "error"); btn.disabled = false; }
            });
        });
        bindFriendButtons(friendsPanel);
    }

    async function loadGlobal(search = "") {
        const data = await window.API.gql(`query($s: String) { users(search: $s, limit: 30) { id username bio avatarEmoji status level plusTier } }`, { s: search });
        globalPanel.innerHTML = data.users.length
            ? data.users.map((u) => personRow(u, { isFriend: false })).join("")
            : `<p class="section-eyebrow">No people found.</p>`;
        bindFriendButtons(globalPanel);
    }

    let searchTimer = null;
    const onSearch = () => {
        clearTimeout(searchTimer);
        searchTimer = setTimeout(() => {
            const q = searchInput ? searchInput.value : "";
            loadFriends(q);
            loadGlobal(q);
        }, 300);
    };
    if (searchInput) searchInput.addEventListener("input", onSearch);
    if (searchBtn) searchBtn.addEventListener("click", onSearch);

    loadFriends();
    loadGlobal();
});
