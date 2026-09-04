//! Landing page: interactive stats-card link generator with live preview.
//!
//! `GET /` previously matched no route and fell through to Axum's 404. It now
//! serves a small self-contained HTML page (inline CSS/JS, no extra assets or
//! dependencies) where visitors can type a Quaver username, pick a key mode,
//! and get a shareable `/card/{name}` link together with a live image preview.

use axum::response::Html;

/// Serves the generator page. Everything is inline so the page has no extra
/// runtime assets that would need startup validation (cf. `card::validate_assets`).
pub async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

// Raw string delimiter is `##` so `"#` sequences (e.g. `href="#..."` or hex
// colours next to a quote) can never terminate the literal early.
const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Quaver Stats Card Generator</title>
<style>
  * { box-sizing: border-box; }
  body {
    margin: 0;
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #15161e;
    color: #c0caf5;
    font-family: "JetBrains Mono", ui-monospace, monospace;
  }
  main {
    width: 100%;
    max-width: 420px;
    padding: 24px;
  }
  h1 { font-size: 1.25rem; margin: 0 0 8px; }
  p.sub { margin: 0 0 20px; color: #7982a9; font-size: 0.85rem; }
  form { display: flex; flex-direction: column; gap: 12px; }
  input[type="text"] {
    width: 100%;
    padding: 10px 12px;
    border: 1px solid #3b4261;
    border-radius: 6px;
    background: #1f2335;
    color: inherit;
    font: inherit;
  }
  input[type="text"]:focus { outline: none; border-color: #7aa2f7; }
  .modes { display: flex; gap: 16px; font-size: 0.9rem; }
  .modes label { display: flex; align-items: center; gap: 6px; cursor: pointer; }
  button {
    padding: 10px 12px;
    border: none;
    border-radius: 6px;
    background: #7aa2f7;
    color: #15161e;
    font: inherit;
    font-weight: bold;
    cursor: pointer;
  }
  button:hover { background: #89b4fa; }
  #result { margin-top: 24px; }
  .link-row { display: flex; gap: 8px; }
  .link-row input { flex: 1; }
  .link-row button { background: #3b4261; color: #c0caf5; font-weight: normal; }
  .link-row button:hover { background: #4a5178; }
  #hint { min-height: 1.2em; margin: 8px 0; font-size: 0.85rem; color: #e0af68; }
  #hint:empty { margin: 0; }
  #preview {
    display: block;
    margin-top: 12px;
    border-radius: 6px;
    border: 1px solid #3b4261;
    background: #1f2335;
  }
  footer { margin-top: 24px; font-size: 0.8rem; color: #565f89; }
  footer code { color: #9ece6a; }
</style>
</head>
<body>
<main>
  <h1>Quaver Stats Card Generator</h1>
  <p class="sub">Enter a Quaver username to generate a stats card link.</p>

  <form id="gen">
    <input id="username" type="text" placeholder="Username (e.g. young0727)"
           autocomplete="off" required>
    <div class="modes">
      <label><input type="radio" name="mode" value="7k" checked> 7K</label>
      <label><input type="radio" name="mode" value="4k"> 4K</label>
    </div>
    <button type="submit">Generate</button>
  </form>

  <section id="result" hidden>
    <div class="link-row">
      <input id="link" type="text" readonly>
      <button id="copy" type="button">Copy</button>
    </div>
    <p id="hint"></p>
    <img id="preview" alt="stats card preview" width="256" height="192" hidden>
  </section>

  <footer>API: <code>GET /card/{username}?mode=7k|4k</code></footer>
</main>
<script>
const form = document.getElementById('gen');
const userInput = document.getElementById('username');
const result = document.getElementById('result');
const linkInput = document.getElementById('link');
const preview = document.getElementById('preview');
const hint = document.getElementById('hint');
const copyBtn = document.getElementById('copy');

form.addEventListener('submit', (e) => {
  e.preventDefault();
  const name = userInput.value.trim();
  if (!name) return;
  const mode = new FormData(form).get('mode');
  const path = '/card/' + encodeURIComponent(name) + '?mode=' + mode;
  linkInput.value = location.origin + path;
  result.hidden = false;
  preview.hidden = true;
  hint.textContent = 'Loading preview…';
  preview.src = path;
});

preview.addEventListener('load', () => {
  hint.textContent = '';
  preview.hidden = false;
});

preview.addEventListener('error', () => {
  preview.hidden = true;
  hint.textContent = 'Could not load the card — check the username, or the upstream API may be down.';
});

copyBtn.addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(linkInput.value);
  } catch {
    linkInput.select();
    document.execCommand('copy');
  }
  copyBtn.textContent = 'Copied!';
  setTimeout(() => { copyBtn.textContent = 'Copy'; }, 1500);
});
</script>
</body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_index_serves_generator_page() {
        let Html(body) = index().await;
        assert!(body.starts_with("<!DOCTYPE html>"));
        // 互動元素：使用者輸入、模式選擇與產生按鈕。
        assert!(body.contains(r##"id="username""##));
        assert!(body.contains(r##"name="mode" value="7k""##));
        assert!(body.contains(r##"name="mode" value="4k""##));
        // 預覽與連結輸出元素。
        assert!(body.contains(r##"id="preview""##));
        assert!(body.contains(r##"id="link""##));
        // 生成的連結必須指向卡片路由。
        assert!(body.contains("/card/"));
    }
}
