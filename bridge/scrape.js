const puppeteer = require("puppeteer");

async function scrape(url) {
  try {
    const browser = await puppeteer.launch({
      args: ["--no-sandbox", "--disable-setuid-sandbox", "--headless"],
    });
    const page = await browser.newPage();

    // Anti-bot detection simple evader
    await page.setUserAgent(
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
    );

    // Go to URL and wait for network to be idle
    await page.goto(url, { waitUntil: "networkidle2", timeout: 30000 });

    // Extract plain text from the body
    const text = await page.evaluate(() => {
      // Remove script and style tags
      const scripts = document.querySelectorAll(
        "script, style, noscript, nav, footer, header",
      );
      scripts.forEach((s) => s.remove());
      return document.body.innerText;
    });

    await browser.close();
    console.log(text.trim());
  } catch (err) {
    console.error("Error scraping:", err.message);
    process.exit(1);
  }
}

const url = process.argv[2];
if (!url) {
  console.error("Please provide a URL.");
  process.exit(1);
}

scrape(url);
