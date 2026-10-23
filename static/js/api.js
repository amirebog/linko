function csrfToken() {
  const match = document.cookie.split("; ").find((row) => row.startsWith("linko_csrf="));
  return match ? decodeURIComponent(match.split("=").slice(1).join("=")) : "";
}

function hideAppLoader() {
  const loader = document.getElementById("app-loader");
  if (loader) loader.hidden = true;
  const main = document.getElementById("main");
  if (main && main.hasAttribute("aria-busy")) {
    main.setAttribute("aria-busy", "false");
  }
}

async function ensureCsrf() {
  if (csrfToken()) return;
  await fetch("/api/auth/csrf", { credentials: "same-origin" });
}

async function api(path, options = {}) {
  await ensureCsrf();
  const headers = Object.assign({ "Content-Type": "application/json" }, options.headers || {});
  const method = (options.method || "GET").toUpperCase();
  if (method !== "GET" && method !== "HEAD") {
    headers["X-CSRF-Token"] = csrfToken();
  }
  const res = await fetch(path, Object.assign({}, options, { headers, credentials: "same-origin" }));
  const text = await res.text();
  let data = null;
  try { data = text ? JSON.parse(text) : null; } catch { data = { raw: text }; }
  if (!res.ok) {
    const message = data && data.error && data.error.message ? data.error.message : "Request failed";
    const error = new Error(message);
    error.status = res.status;
    error.payload = data;
    throw error;
  }
  return data;
}

function toast(message, kind) {
  let wrap = document.querySelector(".toast-wrap");
  if (!wrap) {
    wrap = document.createElement("div");
    wrap.className = "toast-wrap";
    wrap.setAttribute("role", "status");
    document.body.appendChild(wrap);
  }
  const el = document.createElement("div");
  el.className = `toast ${kind || ""}`;
  el.textContent = message;
  wrap.appendChild(el);
  setTimeout(() => el.remove(), 3200);
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function confirmAction(message) {
  return new Promise((resolve) => {
    const back = document.createElement("div");
    back.className = "modal-back";
    back.innerHTML = `<div class="modal" role="dialog" aria-modal="true"><p>${escapeHtml(message)}</p><div class="hero-actions"><button class="btn btn-danger" data-ok>تأیید</button><button class="btn btn-ghost" data-no>انصراف</button></div></div>`;
    document.body.appendChild(back);
    back.querySelector("[data-ok]").onclick = () => { back.remove(); resolve(true); };
    back.querySelector("[data-no]").onclick = () => { back.remove(); resolve(false); };
  });
}

async function signOut() {
  try {
    await api("/api/auth/logout", { method: "POST" });
  } catch (_) {
    /* still leave */
  }
  location.href = "/login";
}

document.querySelectorAll('[data-action="signout"]').forEach((button) => {
  button.addEventListener("click", signOut);
});

document.querySelectorAll('[data-action="toggle-sidebar"]').forEach((button) => {
  button.addEventListener("click", () => {
    const sidebar = document.getElementById("sidebar");
    if (!sidebar) return;
    const isOpen = sidebar.classList.toggle("open");
    button.setAttribute("aria-expanded", String(isOpen));
  });
});
