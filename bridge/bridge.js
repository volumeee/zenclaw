const { Client, LocalAuth } = require("whatsapp-web.js");
const qrcode = require("qrcode-terminal");
const express = require("express");
const bodyParser = require("body-parser");

const app = express();
app.use(bodyParser.json());

const client = new Client({
  authStrategy: new LocalAuth(),
  puppeteer: {
    args: ["--no-sandbox", "--disable-setuid-sandbox"],
  },
});

let messagesBuffer = [];
let lastQr = null;

client.on("qr", (qr) => {
  lastQr = qr;
  // Generate and scan this code with your phone
  console.log("SCAN THIS QR CODE WITH WHATSAPP:");
  qrcode.generate(qr, { small: true });
});

client.on("ready", () => {
  lastQr = null;
  console.log("✅ WhatsApp Bridge is ready!");
});

const handleIncomingMessage = async (msg) => {
  if (msg.from === "status@broadcast") return;

  // Prevent infinite loops: don't process messages sent by the bot unless it's to itself
  // and ensure we don't process the bot's own AI responses.
  if (msg.fromMe && msg.to !== msg.from) return;

  let senderName = msg._data.notifyName || "";
  try {
    if (msg.author) {
      const contact = await client.getContactById(msg.author);
      senderName = contact.pushname || contact.name || senderName;
    } else {
      const contact = await client.getContactById(msg.from);
      senderName = contact.pushname || contact.name || senderName;
    }
  } catch (e) {}

  const payload = {
    id: msg.id.id,
    from: msg.from,
    body: msg.body,
    is_group: msg.from.includes("@g.us"),
    sender_name: senderName,
  };

  messagesBuffer.push(payload);
};

client.on("message", handleIncomingMessage);

client.on("message_create", async (msg) => {
  // message_create captures messages sent by the authenticated user
  if (msg.fromMe && msg.to === msg.from) {
    handleIncomingMessage(msg);
  }
});

// Exposed Endpoints for ZenClaw
app.get("/messages", (req, res) => {
  res.json(messagesBuffer);
  messagesBuffer = []; // Clear after polling
});

app.post("/send", async (req, res) => {
  const { to, message } = req.body;
  try {
    await client.sendMessage(to, message);
    res.json({ success: true });
  } catch (err) {
    console.error("Failed to send message:", err);
    res.status(500).json({ error: err.message });
  }
});

app.get("/status", (req, res) => {
  res.json({ status: "ok", ready: client.info ? true : false });
});

app.get("/qr", (req, res) => {
  res.json({ qr: lastQr });
});

app.listen(3001, () => {
  console.log("🔗 Bridge HTTP server running on port 3001");
});

console.log("🚀 WhatsApp Bridge initializing...");
client.initialize();
