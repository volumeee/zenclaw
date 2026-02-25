# 🚀 ZenClaw Agentic Intelligence Roadmap

Dokumen ini adalah cetak biru (blueprint) untuk meningkatkan kecerdasan `Agent` di ZenClaw dari tahap "Smart Chatbot" menjadi **God-Level Autonomous AI** (setara Devin/Cursor).

Semua implementasi harus mengikuti prinsip dasar ZenClaw:

- **DRY (Don't Repeat Yourself):** Reusable struct & traits.
- **Robustness:** Error handling yang ketat (tidak boleh panic, parsing JSON harus aman).
- **Consistency:** Mengikuti pattern `Tool` trait yang sudah ada di `zenclaw_core::tool::Tool`.

---

## 🎯 Fase 1: Semantic Codebase Awareness (Mata Programmer)

**Problem:** Saat ini agen menebak-nebak nama file menggunakan `ListDirTool` berulang kali. Ini boros token dan waktu.
**Solusi:** Membuat `CodebaseSearchTool` yang terintegrasi dengan utilitas pencarian cepat (seperti `ripgrep` atau walker Rust `ignore`) untuk mencari kata kunci atau definisi fungsi/struct di seluruh codebase dalam hitungan milidetik.

- [x] **Tugas:** Buat `crates/zenclaw-hub/src/tools/code_search.rs`.
- [x] **Spesifikasi:** Harus mendukung parameter `query` (kata yang dicari), `dir` (direktori), dan `file_pattern` (opsional misal `*.rs`).
- [x] **DRY Pattern:** Gunakan library `ignore` atau `grep` native Rust agar tidak bergantung pada binary eksternal OS, memastikan cross-platform (Windows/Linux/Mac).

---

## 🎯 Fase 2: Auto-Linter & Self-Correction (Self-Healing Code)

**Problem:** Agen menulis kode (via `WriteFileTool`), tapi tidak tahu kodenya bisa dicompile atau tidak sampai user nge-run kodenya.
**Solusi:** `LinterTool` yang bisa mendeteksi bahasa pemrograman dan otomatis memvalidasi _syntax_ secara _background_.

- [ ] **Tugas:** Buat `crates/zenclaw-hub/src/tools/linter.rs`.
- [ ] **Spesifikasi:** Tool ini akan menjalankan linter bawaan (contoh: `cargo check` untuk Rust, `tsc --noEmit` untuk TS, `python -m py_compile` untuk Python).
- [ ] **Pattern:** Integrasikan dengan `EventBus` sistem agar terminal memunculkan Spinner "🛠️ Validating code..." secara halus ke user. Jika linter menemukan error, pesan error dikembalikan ke LLM agar LLM mengoreksinya sendiri di iterasi loop berikutnya!

---

## 🎯 Fase 3: Plan-and-Execute Architecture (Mencegah Halusinasi)

**Problem:** Untuk tugas yang melibatkan 10 langkah (misal "Buat website kalkulator"), agen langsung mencoba ngoding file pertama lalu bingung selanjutnya harus apa.
**Solusi:** Memaksa agen untuk mendeklarasikan Rencana (Plan) secara terstruktur sebelum beraksi.

- [ ] **Tugas:** Upgrade The ReAct Loop di `crates/zenclaw-core/src/agent.rs` atau buat `PlannerTool` khusus.
- [ ] **Spesifikasi:** Memperkenalkan state `Planning` vs `Executing`. Saat user memberi `prompt` besar, agen diinstruksikan via System Prompt: _"Jika tugas kompleks, gunakan PlannerTool untuk membuat daftar langkah JSON. Lalu jalankan satu per satu."_
- [ ] **Robustness:** Menyimpan _active plan_ ke dalam SQLite Memory supaya kalau LLM lupa, sistem mengingatkan "Anda sedang berada di langkah ke-3 dari 5".

---

## 🎯 Fase 4: Vector RAG Memory (Long-Term Recall)

**Problem:** Saat ini `MemoryStore` hanya memotong pesan lama (_truncate_) jika token melebihi batas. Konteks lama hilang selamanya.
**Solusi:** Memasang Semantic/Vector Database embedding ringan (seperti `sqlpage` atau komputasi embedding simpel lokal).

- [ ] **Tugas:** Upgrade trait `MemoryStore` di `crates/zenclaw-core/src/memory.rs`.
- [ ] **Spesifikasi:** Setiap turn percakapan yang masuk ke `SQLite` tidak hanya disimpan raw text-nya, tetapi juga dicari "similarity"-nya saat user menanyakan sesuatu tentang masa lalu. Ini fitur _God-Tier_ karena agen tidak akan pernah lupa kode yang ditulis minggu lalu.

---

## 🎯 Fase 5: Headless Browser Actions (Mata Internet)

**Problem:** `WebScrapeTool` saat ini hanya bisa membaca web statis (HTML biasa), gagal di web React/SPA atau yang dilindungi Cloudflare.
**Solusi:** Integrasi Playwright/Puppeteer via Node.js bridge atau native `headless_chrome`.

- [ ] **Tugas:** Upgrade `crates/zenclaw-hub/src/tools/web_scrape.rs`.
- [ ] **Spesifikasi:** Menggunakan browser engine asli di belakang layar sehingga bisa mengeksekusi JavaScript, menunggu elemen muncul, atau bahkan mengklik tombol persetujuan cookie.
