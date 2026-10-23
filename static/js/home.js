document.addEventListener("DOMContentLoaded", () => {
  const stage = document.getElementById("preview-stage");
  if (!stage) return;
  const samples = [
    { name: "Nora", role: "Product designer", theme: "مینیمال" },
    { name: "Kasra", role: "Systems engineer", theme: "دولوپر" },
    { name: "Leila", role: "Photographer", theme: "کریتور" },
  ];
  let i = 0;
  const render = () => {
    const s = samples[i % samples.length];
    stage.querySelector("[data-name]").textContent = s.name;
    stage.querySelector("[data-role]").textContent = s.role;
    stage.querySelector("[data-theme]").textContent = s.theme;
  };
  render();
  setInterval(() => { i += 1; render(); }, 3800);
});
