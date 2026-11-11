// Kiểm tra bố cục giao diện (chạy trong trình duyệt, không cần backend).
//
// Cách dùng:
//   1. `npm run dev`, mở http://localhost:1420/?demo&page=dashboard
//   2. Đặt kích thước cửa sổ cần thử (nên thử: 860x560 = nhỏ nhất app cho phép, 1100x720, 1366x768, 1920x1080)
//   3. Dán toàn bộ file này vào Console rồi chạy `await layoutCheck()`
//
// Với mọi trang ở thanh bên, ở cả theme tối và sáng, báo:
//   - trang cuộn ngang; phần tử tràn ra ngoài vùng nội dung (bỏ qua phần nằm trong vùng cuộn riêng);
//   - nút / ô nhập / ô chọn đè lên nhau; chữ trong nút bị cắt.
// `issues` rỗng = sạch. (Đã thử: chèn phần tử rộng 3000px và 2 nút đè nhau => bắt được cả 3 loại lỗi.)

async function layoutCheck() {
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const main = document.querySelector("main");
  const vis = () => [...main.children].find((d) => !d.hidden);
  const navButtons = [...document.querySelectorAll("nav button")].filter((b) => b.textContent.trim());
  const issues = [];

  const check = (name, theme) => {
    const v = vis();
    const mr = main.getBoundingClientRect();
    if (main.scrollWidth > main.clientWidth + 1) issues.push(`${name}/${theme}: trang cuộn ngang (${main.scrollWidth} > ${main.clientWidth})`);
    for (const el of v.querySelectorAll("*")) {
      const r = el.getBoundingClientRect();
      if (!r.width || !r.height) continue;
      let p = el.parentElement;
      let clipped = false;
      while (p && p !== main) {
        if (/(auto|scroll|hidden|clip)/.test(getComputedStyle(p).overflowX)) {
          clipped = true;
          break;
        }
        p = p.parentElement;
      }
      if (!clipped && (r.right > mr.right + 1 || r.left < mr.left - 1)) {
        issues.push(`${name}/${theme}: tràn ngang <${el.tagName.toLowerCase()}> "${(el.textContent || "").trim().slice(0, 40)}"`);
        break;
      }
    }
    const ctrls = [...v.querySelectorAll("button, input:not([type=checkbox]), select")].filter((e) => {
      const r = e.getBoundingClientRect();
      return r.width > 0 && r.height > 0;
    });
    for (let i = 0; i < ctrls.length; i++)
      for (let j = i + 1; j < ctrls.length; j++) {
        const a = ctrls[i];
        const b = ctrls[j];
        if (a.contains(b) || b.contains(a)) continue;
        const ra = a.getBoundingClientRect();
        const rb = b.getBoundingClientRect();
        const w = Math.min(ra.right, rb.right) - Math.max(ra.left, rb.left);
        const h = Math.min(ra.bottom, rb.bottom) - Math.max(ra.top, rb.top);
        if (w > 2 && h > 2) issues.push(`${name}/${theme}: đè nhau "${a.textContent.trim().slice(0, 20) || a.tagName}" × "${b.textContent.trim().slice(0, 20) || b.tagName}"`);
      }
    for (const b of v.querySelectorAll("button"))
      if (b.scrollWidth > b.clientWidth + 1 && b.getBoundingClientRect().width > 0) issues.push(`${name}/${theme}: chữ trong nút bị cắt "${b.textContent.trim().slice(0, 30)}"`);
  };

  const original = document.documentElement.dataset.theme;
  for (const theme of ["dark", "light"]) {
    document.documentElement.dataset.theme = theme;
    for (const btn of navButtons) {
      const name = btn.textContent.trim();
      btn.click();
      await wait(500);
      // Trạng thái có dữ liệu cho trang Dọn rác.
      if (name.startsWith("Dọn rác")) {
        const scan = [...vis().querySelectorAll("button")].find((x) => /^(Quét ngay|Quét lại)$/.test(x.textContent.trim()));
        if (scan) {
          scan.click();
          await wait(500);
        }
      }
      check(name, theme);
    }
  }
  document.documentElement.dataset.theme = original;
  return { size: `${innerWidth}x${innerHeight}`, issues: [...new Set(issues)] };
}
