// Noobietoria — login / register (wired to the GraphQL backend)
document.addEventListener("DOMContentLoaded", () => {
    // ----- show / hide password toggles -----
    document.querySelectorAll("[data-toggle-password]").forEach((btn) => {
        const input = document.getElementById(btn.dataset.togglePassword);
        if (!input) return;

        btn.addEventListener("click", () => {
            const showing = input.type === "text";
            input.type = showing ? "password" : "text";
            btn.textContent = showing ? "SHOW" : "HIDE";
        });
    });

    // ----- helpers -----
    const setError = (input, message) => {
        input.classList.add("is-invalid");
        const hint = document.querySelector(`[data-error-for="${input.id}"]`);
        if (hint) {
            hint.textContent = message;
            hint.classList.add("is-visible");
        }
    };

    const clearError = (input) => {
        input.classList.remove("is-invalid");
        const hint = document.querySelector(`[data-error-for="${input.id}"]`);
        if (hint) {
            hint.textContent = "";
            hint.classList.remove("is-visible");
        }
    };

    // clear error state while typing
    document.querySelectorAll(".auth__input").forEach((input) => {
        input.addEventListener("input", () => clearError(input));
    });

    // ----- spinner overlay (markup comes from start.css .loader-overlay) -----
    const loaderOverlay = document.getElementById("page-loader");
    const loaderText = loaderOverlay ? loaderOverlay.querySelector("[data-loader-text]") : null;

    const showLoader = (text) => {
        if (!loaderOverlay) return;
        if (loaderText && text) loaderText.textContent = text;
        loaderOverlay.classList.add("is-visible");
    };

    const hideLoader = () => {
        if (loaderOverlay) loaderOverlay.classList.remove("is-visible");
    };

    const submitError = (form, message) => {
        hideLoader();
        const btn = form.querySelector('[type="submit"]');
        if (btn) btn.disabled = false;
        window.API.toast(message, "error");
    };

    // ----- already logged in? go straight to the app -----
    if (window.API.token) {
        window.location.href = "home.html";
        return;
    }

    // ----- social login buttons (Google / Discord / Roblox via OAuth 2.0) -----
    const socialContainer = document.querySelector(".auth__social");
    const socialDivider = document.querySelector(".auth__divider");
    const socialButtons = document.querySelectorAll("[data-social]");

    // Check backend provider configuration and hide unconfigured buttons
    if (socialButtons.length > 0) {
        fetch((window.NOOBIE_API_URL || "") + "/auth/providers")
            .then((res) => (res.ok ? res.json() : null))
            .catch(() => null)
            .then((providers) => {
                if (!providers) return;

                let visibleCount = 0;
                socialButtons.forEach((btn) => {
                    const provider = btn.dataset.social;
                    const isConfigured = !!providers[provider];
                    if (!isConfigured) {
                        btn.style.display = "none";
                    } else {
                        btn.style.display = "";
                        visibleCount++;
                    }
                });

                if (socialContainer) {
                    socialContainer.setAttribute("data-visible-count", String(visibleCount));
                }

                if (visibleCount === 0) {
                    if (socialContainer) socialContainer.style.display = "none";
                    if (socialDivider) socialDivider.style.display = "none";
                } else {
                    if (socialContainer) socialContainer.style.display = "grid";
                    if (socialDivider) socialDivider.style.display = "flex";
                }
            });
    }

    socialButtons.forEach((btn) => {
        btn.addEventListener("click", () => {
            const provider = btn.dataset.social; // "discord" | "google" | "roblox"
            showLoader("Connecting to " + provider.charAt(0).toUpperCase() + provider.slice(1) + "...");
            window.location.href = (window.NOOBIE_API_URL || "") + "/auth/" + provider;
        });
    });

    // ----- came back from a provider with an error? -----
    const oauthError = new URLSearchParams(window.location.search).get("oauth_error");
    if (oauthError) {
        const pretty = oauthError.replace(/_/g, " ").trim();
        window.API.toast("Social login failed: " + pretty, "error");
    }

    // ----- login form -----
    const loginForm = document.getElementById("login-form");
    if (loginForm) {
        loginForm.addEventListener("submit", async (e) => {
            e.preventDefault();

            const id = document.getElementById("login-id");
            const password = document.getElementById("login-password");
            let valid = true;

            if (!id.value.trim()) {
                setError(id, "Please enter your username or email.");
                valid = false;
            }
            if (password.value.length < 6) {
                setError(password, "Password must be at least 6 characters.");
                valid = false;
            }
            if (!valid) return;

            loginForm.querySelector('[type="submit"]').disabled = true;
            showLoader("Logging you in...");
            try {
                const data = await window.API.gql(
                    `mutation($u:String!,$p:String!){ login(usernameOrEmail:$u,password:$p){ token } }`,
                    { u: id.value.trim(), p: password.value },
                );
                window.API.token = data.login.token;
                window.location.href = "home.html";
            } catch (err) {
                setError(password, "");
                setError(id, err.message);
                submitError(loginForm, err.message);
            }
        });
    }

    // ----- register form -----
    const registerForm = document.getElementById("register-form");
    if (registerForm) {
        registerForm.addEventListener("submit", async (e) => {
            e.preventDefault();

            const username = document.getElementById("reg-username");
            const email = document.getElementById("reg-email");
            const dob = document.getElementById("reg-dob");
            const password = document.getElementById("reg-password");
            const confirm = document.getElementById("reg-confirm");
            const terms = document.getElementById("reg-terms");
            let valid = true;

            if (username.value.trim().length < 3) {
                setError(username, "Username must be at least 3 characters.");
                valid = false;
            }
            if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.value.trim())) {
                setError(email, "Please enter a valid email address.");
                valid = false;
            }
            if (!dob.value) {
                setError(dob, "Please enter your date of birth.");
                valid = false;
            } else {
                const birthDate = new Date(dob.value);
                const today = new Date();
                let age = today.getFullYear() - birthDate.getFullYear();
                const m = today.getMonth() - birthDate.getMonth();
                if (m < 0 || (m === 0 && today.getDate() < birthDate.getDate())) {
                    age--;
                }
                if (age < 13) {
                    setError(dob, "You must be at least 13 years old to join Noobietoria.");
                    valid = false;
                }
            }
            if (password.value.length < 6) {
                setError(password, "Password must be at least 6 characters.");
                valid = false;
            }
            if (confirm.value !== password.value || !confirm.value) {
                setError(confirm, "Passwords do not match.");
                valid = false;
            }
            if (!terms.checked) {
                const hint = document.querySelector('[data-error-for="reg-terms"]');
                if (hint) {
                    hint.textContent = "You need to accept the Terms to continue.";
                    hint.classList.add("is-visible");
                }
                valid = false;
            }
            if (!valid) return;

            registerForm.querySelector('[type="submit"]').disabled = true;
            showLoader("Creating your account...");
            try {
                const data = await window.API.gql(
                    `mutation($u:String!,$e:String!,$p:String!,$d:String){ register(username:$u,email:$e,password:$p,dateOfBirth:$d){ token } }`,
                    {
                        u: username.value.trim(),
                        e: email.value.trim(),
                        p: password.value,
                        d: dob.value || null,
                    },
                );
                window.API.token = data.register.token;
                window.location.href = "home.html";
            } catch (err) {
                submitError(registerForm, err.message);
            }
        });

        // hide the terms error as soon as the box is ticked
        const terms = document.getElementById("reg-terms");
        terms.addEventListener("change", () => {
            const hint = document.querySelector('[data-error-for="reg-terms"]');
            if (terms.checked && hint) hint.classList.remove("is-visible");
        });
    }
});