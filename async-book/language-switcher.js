(function () {
  function getInfo() {
    var path = window.location.pathname;
    var marker = "/async-book/zh/";
    var isZh = path.indexOf(marker) !== -1;
    var chapter = "index.html";

    if (isZh) {
      chapter = path.slice(path.indexOf(marker) + marker.length) || "index.html";
      return {
        current: "zh",
        enHref: "../" + chapter,
        zhHref: chapter,
      };
    }

    var base = "/async-book/";
    var index = path.indexOf(base);
    if (index !== -1) {
      chapter = path.slice(index + base.length) || "index.html";
    }
    return {
      current: "en",
      enHref: chapter,
      zhHref: "zh/" + chapter,
    };
  }

  function addSwitcher() {
    var menu = document.getElementById("menu-bar");
    if (!menu || document.querySelector(".language-switcher")) {
      return;
    }

    var info = getInfo();
    var switcher = document.createElement("div");
    switcher.className = "language-switcher";
    switcher.innerHTML =
      '<a class="' + (info.current === "en" ? "active" : "") + '" href="' + info.enHref + '">EN</a>' +
      '<a class="' + (info.current === "zh" ? "active" : "") + '" href="' + info.zhHref + '">中文</a>';

    menu.appendChild(switcher);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", addSwitcher);
  } else {
    addSwitcher();
  }
})();
