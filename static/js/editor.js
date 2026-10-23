const DEFAULTS = {
  heading: { text: "عنوان جدید", level: 2 },
  paragraph: { text: "متن خود را اینجا بنویسید." },
  link: { label: "وب‌سایت", url: "https://example.com" },
  social_links: { items: [{ network: "github", label: "GitHub", url: "https://github.com" }] },
  divider: {},
  image: { url: "https://picsum.photos/800/400", alt: "تصویر نمونه" },
  cta: { label: "تماس بگیرید", url: "https://example.com" },
  project_card: { title: "پروژه", description: "توضیح کوتاه", url: "https://example.com", image_url: "" },
  skills: { items: ["Rust", "UI"] },
  contact: { email: "hello@example.com", phone: "", location: "" },
};

let blocks = [];
let dragId = null;

function fieldsFor(block) {
  const c = block.content || JSON.parse(block.content_json || "{}");
  const type = block.block_type || block.type;
  if (type === "heading") {
    return `<label>متن</label><input data-k="text" value="${escapeHtml(c.text || "")}">
      <label>سطح</label><select data-k="level"><option ${c.level==1?"selected":""}>1</option><option ${c.level==2?"selected":""}>2</option><option ${c.level==3?"selected":""}>3</option></select>`;
  }
  if (type === "paragraph") return `<label>متن</label><textarea data-k="text">${escapeHtml(c.text || "")}</textarea>`;
  if (type === "link" || type === "cta") {
    return `<label>برچسب</label><input data-k="label" value="${escapeHtml(c.label || "")}">
      <label>آدرس</label><input data-k="url" value="${escapeHtml(c.url || "")}">`;
  }
  if (type === "image") {
    return `<label>آدرس تصویر</label><input data-k="url" value="${escapeHtml(c.url || "")}">
      <label>متن جایگزین</label><input data-k="alt" value="${escapeHtml(c.alt || "")}">`;
  }
  if (type === "project_card") {
    return `<label>عنوان</label><input data-k="title" value="${escapeHtml(c.title || "")}">
      <label>توضیح</label><textarea data-k="description">${escapeHtml(c.description || "")}</textarea>
      <label>آدرس</label><input data-k="url" value="${escapeHtml(c.url || "")}">
      <label>تصویر</label><input data-k="image_url" value="${escapeHtml(c.image_url || "")}">`;
  }
  if (type === "skills") {
    return `<label>مهارت‌ها (با ویرگول)</label><input data-k="items" value="${escapeHtml((c.items || []).join(", "))}">`;
  }
  if (type === "social_links") {
    const text = (c.items || []).map((i) => `${i.network}|${i.label}|${i.url}`).join("\n");
    return `<label>هر خط: شبکه|برچسب|آدرس</label><textarea data-k="items">${escapeHtml(text)}</textarea>`;
  }
  if (type === "contact") {
    return `<label>ایمیل</label><input data-k="email" value="${escapeHtml(c.email || "")}">
      <label>تلفن</label><input data-k="phone" value="${escapeHtml(c.phone || "")}">
      <label>موقعیت</label><input data-k="location" value="${escapeHtml(c.location || "")}">`;
  }
  return `<p class="muted">این بلوک تنظیمات متنی ندارد.</p>`;
}

function readContent(card, type) {
  const get = (k) => card.querySelector(`[data-k="${k}"]`);
  if (type === "heading") return { text: get("text").value, level: Number(get("level").value) };
  if (type === "paragraph") return { text: get("text").value };
  if (type === "link" || type === "cta") return { label: get("label").value, url: get("url").value };
  if (type === "image") return { url: get("url").value, alt: get("alt").value };
  if (type === "project_card") {
    return {
      title: get("title").value,
      description: get("description").value,
      url: get("url").value,
      image_url: get("image_url").value,
    };
  }
  if (type === "skills") {
    return { items: get("items").value.split(",").map((s) => s.trim()).filter(Boolean) };
  }
  if (type === "social_links") {
    const items = get("items").value.split("\n").map((line) => line.trim()).filter(Boolean).map((line) => {
      const [network, label, url] = line.split("|");
      return { network: network || "web", label: label || network, url: url || "https://example.com" };
    });
    return { items };
  }
  if (type === "contact") {
    return { email: get("email").value, phone: get("phone").value, location: get("location").value };
  }
  return {};
}

function render() {
  const root = document.getElementById("block-list");
  if (!blocks.length) {
    root.innerHTML = `<div class="empty-state"><h2>صفحه هنوز خالی است</h2><p data-i18n="emptyBlocks">${I18N[currentLang()].emptyBlocks}</p></div>`;
    applyI18n();
    return;
  }
  root.innerHTML = blocks.map((block) => {
    const type = block.block_type || block.type;
    const content = block.content || JSON.parse(block.content_json || "{}");
    block.content = content;
    return `<article class="block-item" draggable="true" data-id="${block.id}">
      <div class="block-toolbar">
        <strong>${escapeHtml(type)}</strong>
        <button type="button" data-act="save">ذخیره</button>
        <button type="button" data-act="dup">تکثیر</button>
        <button type="button" data-act="vis">${block.is_visible ? "مخفی" : "نمایان"}</button>
        <button type="button" data-act="up">بالا</button>
        <button type="button" data-act="down">پایین</button>
        <button type="button" data-act="del">حذف</button>
      </div>
      ${fieldsFor(block)}
    </article>`;
  }).join("");
}

async function refresh() {
  blocks = await api("/api/pages/me/blocks");
  blocks.forEach((b) => { b.content = JSON.parse(b.content_json); });
  render();
}

async function persistOrder() {
  await api("/api/pages/me/blocks/reorder", {
    method: "POST",
    body: JSON.stringify({ ids: blocks.map((b) => b.id) }),
  });
}

async function addBlock(type) {
  try {
    await api("/api/pages/me/blocks", {
      method: "POST",
      body: JSON.stringify({ type, content: DEFAULTS[type] }),
    });
    toast("بلوک اضافه شد", "ok");
    await refresh();
  } catch (err) {
    toast(err.message, "err");
  }
}

document.getElementById("add-block").addEventListener("click", () => {
  addBlock(document.getElementById("block-type").value);
});

document.querySelectorAll("[data-add-type]").forEach((button) => {
  button.addEventListener("click", () => {
    const type = button.dataset.addType;
    document.getElementById("block-type").value = type;
    addBlock(type);
  });
});

document.getElementById("block-list").addEventListener("click", async (event) => {
  const btn = event.target.closest("[data-act]");
  if (!btn) return;
  const card = btn.closest(".block-item");
  const id = card.dataset.id;
  const block = blocks.find((b) => b.id === id);
  const type = block.block_type || block.type;
  const act = btn.dataset.act;
  try {
    if (act === "save") {
      await api(`/api/pages/me/blocks/${id}`, {
        method: "PATCH",
        body: JSON.stringify({ content: readContent(card, type) }),
      });
      toast("ذخیره شد", "ok");
    } else if (act === "dup") {
      await api(`/api/pages/me/blocks/${id}/duplicate`, { method: "POST" });
    } else if (act === "vis") {
      await api(`/api/pages/me/blocks/${id}`, {
        method: "PATCH",
        body: JSON.stringify({ is_visible: !block.is_visible }),
      });
    } else if (act === "del") {
      if (!(await confirmAction("این بلوک حذف شود؟"))) return;
      await api(`/api/pages/me/blocks/${id}`, { method: "DELETE" });
    } else if (act === "up" || act === "down") {
      const i = blocks.findIndex((b) => b.id === id);
      const j = act === "up" ? i - 1 : i + 1;
      if (j < 0 || j >= blocks.length) return;
      const tmp = blocks[i];
      blocks[i] = blocks[j];
      blocks[j] = tmp;
      await persistOrder();
    }
    await refresh();
  } catch (err) {
    toast(err.message, "err");
  }
});

document.getElementById("block-list").addEventListener("dragstart", (event) => {
  const item = event.target.closest(".block-item");
  if (!item) return;
  dragId = item.dataset.id;
  item.classList.add("dragging");
});
document.getElementById("block-list").addEventListener("dragend", (event) => {
  const item = event.target.closest(".block-item");
  if (item) item.classList.remove("dragging");
});
document.getElementById("block-list").addEventListener("dragover", (event) => {
  event.preventDefault();
});
document.getElementById("block-list").addEventListener("drop", async (event) => {
  event.preventDefault();
  const target = event.target.closest(".block-item");
  if (!target || !dragId) return;
  const from = blocks.findIndex((b) => b.id === dragId);
  const to = blocks.findIndex((b) => b.id === target.dataset.id);
  if (from < 0 || to < 0 || from === to) return;
  const [moved] = blocks.splice(from, 1);
  blocks.splice(to, 0, moved);
  try {
    await persistOrder();
    await refresh();
  } catch (err) {
    toast(err.message, "err");
  }
});

async function boot() {
  try {
    await api("/api/auth/me");
    await refresh();
  } catch (err) {
    if (err.status === 401) location.href = "/login";
    else toast(err.message, "err");
  } finally {
    hideAppLoader();
  }
}

document.addEventListener("DOMContentLoaded", boot);
