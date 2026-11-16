// Shows the newest PrintZilla version (from the GitHub releases) next to the downloads.
// Without a connection the page simply keeps its normal text.
(function () {
  var targets = document.querySelectorAll("[data-release-version]");
  if (!targets.length || !window.fetch) return;
  fetch("https://api.github.com/repos/Marwaeck/printzilla/releases/latest", {
    headers: { Accept: "application/vnd.github+json" }
  })
    .then(function (r) { return r.ok ? r.json() : null; })
    .then(function (rel) {
      if (!rel || !rel.tag_name) return;
      var version = rel.tag_name.replace(/^v/, "");
      var date = rel.published_at ? new Date(rel.published_at).toLocaleDateString("de-DE") : "";
      var text = "Neueste Version " + version + (date ? " vom " + date : "");
      targets.forEach(function (el) { el.textContent = text; });
    })
    .catch(function () {});
})();
