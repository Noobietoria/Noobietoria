// Noobietoria — profile page (?u=username, defaults to the logged-in user).
document.addEventListener("DOMContentLoaded", async () => {
    if (!window.API || !window.API.requireAuth()) return;

    const nameEl = document.querySelector(".profile-head__name");
    const banner = document.querySelector(".profile-banner");
    const statBoxes = document.querySelectorAll(".stat-box");
    const friendsHead = document.querySelector(".friends-panel__head h2");
    const friendsGrid = document.querySelector(".friends-grid");
    const username = window.API.queryParam("u");

    try {
        const data = await window.API.gql(`query($username: String!) {
            me { id username }
            profile: user(username: $username) {
                id username avatarEmoji level plusTier status lastSeen
                itemSales networth friendsCount streak longestStreak
                createdAt worldsCount bannerColor
            }
        }`, { username: username || "" });

        const profile = data.profile || data.me;
        if (!profile) return;
        const isSelf = profile.id === data.me.id;

        document.title = `${profile.username} - Noobietoria`;
        if (banner) {
            banner.style.background = profile.bannerColor
                ? `linear-gradient(135deg, ${profile.bannerColor}, #17171b)` : "";
        }

        if (nameEl) {
            const badges = [
                `<span class="profile-head__badge">⭐ Level ${profile.level}</span>`,
                profile.plusTier ? `<span class="profile-head__badge">✦ ${profile.plusTier === "plus_deluxe" ? "Plus Deluxe" : "Plus"}</span>` : "",
            ].join(" ");
            nameEl.innerHTML =
                `<span class="av ${window.API.avatarClass(profile.id)}" style="vertical-align:middle;margin-right:10px;">${window.API.escapeHtml(profile.avatarEmoji)}</span>` +
                `${window.API.escapeHtml(profile.username)} ${badges}` +
                `<div data-add-friend-slot style="margin-top:10px;"></div>`;
        }

        if (statBoxes.length >= 3) {
            const online = profile.status !== "offline";
            statBoxes[0].innerHTML = `
                <div class="stat-row"><span class="stat-row__label">🛒 Item sales</span><span class="stat-row__value">${profile.itemSales}</span></div>
                <div class="stat-row"><span class="stat-row__label">📶 Last seen</span><span class="stat-row__value"><span class="status-dot" ${window.API.statusDot(profile.status)}></span><span class="${online ? "online" : ""}">${online ? (profile.status === "ingame" ? "In game" : "Online") : window.API.timeAgo(profile.lastSeen)}</span></span></div>
                <div class="stat-row"><span class="stat-row__label">💼 Avg. Networth</span><span class="stat-row__value">▣ ${window.API.formatCount(profile.networth)}</span></div>
                <div class="stat-row"><span class="stat-row__label">👤 Friends</span><span class="stat-row__value">${profile.friendsCount}</span></div>`;
            statBoxes[1].innerHTML = `
                <div class="stat-row"><span class="stat-row__label">Current Streak</span><span class="stat-row__value streak">🔥 ${profile.streak} days</span></div>
                <div class="stat-row"><span class="stat-row__label">Longest Streak</span><span class="stat-row__value streak">🏆 ${profile.longestStreak} days</span></div>`;
            statBoxes[2].innerHTML = `
                <div class="stat-row"><span class="stat-row__label">Joined</span><span class="stat-row__value">${window.API.formatDate(profile.createdAt)}</span></div>
                <div class="stat-row"><span class="stat-row__label">Worlds created</span><span class="stat-row__value">${profile.worldsCount}</span></div>`;
        }

        // add-friend button (only on other users' profiles)
        const slot = document.querySelector("[data-add-friend-slot]");
        if (slot && !isSelf) {
            const mine = await window.API.gql(`query { friends { id } }`);
            if (!mine.friends.some((f) => f.id === profile.id)) {
                const btn = document.createElement("button");
                btn.className = "btn btn--blue btn--sm";
                btn.textContent = "＋ Add Friend";
                btn.addEventListener("click", async () => {
                    btn.disabled = true;
                    try {
                        await window.API.gql(`mutation($id: ID!){ sendFriendRequest(userId: $id) }`, { id: profile.id });
                        btn.textContent = "Request sent ✓";
                        window.API.toast("Friend request sent to " + profile.username);
                    } catch (err) {
                        btn.disabled = false;
                        window.API.toast(err.message, "error");
                    }
                });
                slot.appendChild(btn);
            }
        }

        // friends grid (the viewer's own friend list; hidden on other profiles)
        if (friendsGrid) {
            if (isSelf) {
                const friends = await window.API.gql(`query { friends { id username avatarEmoji status } }`);
                if (friendsHead) {
                    friendsHead.innerHTML = `Friends <span style="color:var(--muted);font-size:1rem;">(${friends.friends.length})</span>`;
                }
                friendsGrid.innerHTML = friends.friends.map((f) => `
                    <a class="friend-tile" href="profile.html?u=${encodeURIComponent(f.username)}">
                        <span class="av ${window.API.avatarClass(f.id)}">${window.API.escapeHtml(f.avatarEmoji)}</span>
                        <span class="friend-tile__name">${window.API.escapeHtml(f.username)}</span>
                    </a>`).join("");
            } else if (friendsHead) {
                friendsHead.textContent = "Friends";
                friendsGrid.innerHTML = "";
            }
        }
    } catch (err) {
        window.API.toast("Could not load profile: " + err.message, "error");
    }
});
