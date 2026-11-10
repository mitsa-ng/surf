// Content script injected into matching pages at document idle.
document.documentElement.dataset.surfInjected = "yes";
const banner = document.createElement("div");
banner.id = "__surf_test_banner";
banner.textContent = "Surf Test extension injected this banner.";
banner.style.cssText =
  "position:fixed;left:8px;bottom:8px;z-index:2147483647;background:#0a7fa8;color:#fff;" +
  "font:12px -apple-system,sans-serif;padding:6px 10px;border-radius:6px;opacity:.92;";
document.body ? document.body.appendChild(banner) : document.addEventListener("DOMContentLoaded", () => document.body.appendChild(banner));
