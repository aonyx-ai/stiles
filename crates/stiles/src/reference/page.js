const search = document.querySelector("input");
const rules = [...document.querySelectorAll("details")];
const buttons = [...document.querySelectorAll("button")];
const count = document.querySelector("output");
const empty = document.querySelector(".empty");
let language = "";

function apply() {
  const needle = search.value.trim().toLowerCase();
  let shown = 0;
  for (const rule of rules) {
    const languages = rule.dataset.languages.split(" ");
    const match =
      rule.dataset.search.includes(needle) &&
      (language === "" || languages.includes(language));
    rule.hidden = !match;
    shown += match ? 1 : 0;
  }
  count.textContent = shown === 1 ? "1 rule" : `${shown} rules`;
  empty.hidden = shown > 0;
}

function openFromHash() {
  const rule = document.getElementById(decodeURIComponent(location.hash.slice(1)));
  if (rule instanceof HTMLDetailsElement) {
    rule.open = true;
    rule.scrollIntoView();
  }
}

search.addEventListener("input", apply);
for (const button of buttons) {
  button.addEventListener("click", () => {
    language = button.dataset.language;
    for (const other of buttons) {
      other.setAttribute("aria-pressed", String(other === button));
    }
    apply();
  });
}
for (const rule of rules) {
  rule.addEventListener("toggle", () => {
    if (rule.open) {
      history.replaceState(null, "", `#${rule.id}`);
    }
  });
}
addEventListener("hashchange", openFromHash);
apply();
openFromHash();
