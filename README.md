# 📘 คู่มือการใช้งาน 
  Version2 เพิ่มการเชื่อมโยงกับ pkderiv
  📝 สรุปส่วนที่เพิ่มเข้ามา:
  1. Cargo.toml 📦
    # ✨ เพิ่มใหม่
    tower-http = { version = "0.5", features = ["cors"] }
    เหตุผล: เพื่อรองรับ CORS ให้ pkderiv.shop เรียกได้

   2. Rust Code (main.rs) 🦀
     // ✨ เพิ่มใหม่: import CORS
    use tower_http::cors::{CorsLayer, Any}; 
    CORS Configuration
    // ✨✨✨ เพิ่มใหม่: CORS Layer
   let cors = CorsLayer::new()
    .allow_origin(Any)      // อนุญาตทุก domain
    .allow_methods(Any)     // อนุญาตทุก method
    .allow_headers(Any);    // อนุญาตทุก headers
   // เพิ่ม CORS เข้า Router
    let app = Router::new()
    .route("/", get(root_handler))
    .route("/health", get(health_handler))
    .route("/ws", get(ws_handler))
    .layer(cors);  // ✨ เพิ่มบรรทัดนี้
  //Bind Address  
   // ✨ สำคัญ: ใช้ 0.0.0.0 แทน 127.0.0.1
  let addr = "0.0.0.0:3000";  // รับ connection จากภายนอกได้

  3. HTML Code (test.html) 🌐
     Auto-detect WebSocket URL 
      // ✨✨✨ เพิ่มใหม่: Auto-detect
const wsUrl = window.location.hostname === 'localhost' 
    ? 'ws://localhost:3000/ws'      // ทดสอบ local
    : 'wss://pkderiv.shop/ws';      // production
```

---

## 🎯 ทำไมต้องเพิ่มส่วนเหล่านี้?

| ส่วนที่เพิ่ม | เหตุผล |
|------------|--------|
| **CORS** | ให้ website จาก domain อื่น (pkderiv.shop) เรียก API ได้ |
| **0.0.0.0** | รับ connection จากภายนอก EC2 ได้ (ถ้าใช้ 127.0.0.1 จะรับแค่ local) |
| **Auto-detect URL** | HTML รู้ว่าต้องเชื่อมไปที่ localhost หรือ production |
| **wss://** | ใช้ WebSocket Secure (SSL) สำหรับ production |

---

## 🔄 Flow การทำงาน:
```
1. HTML อยู่ที่ pkderiv.shop
   ↓
2. ตรวจสอบ hostname → ไม่ใช่ localhost
   ↓
3. ใช้ URL: wss://pkderiv.shop/ws
   ↓
4. Request ผ่าน Nginx (port 443) + SSL
   ↓
5. Nginx forward ไป Rust (port 3000)
   ↓
6. Rust มี CORS อนุญาต → เชื่อมต่อสำเร็จ! ✅
```

---

## ✅ ทดสอบ:

### **Local (ก่อน deploy):**
```
เปิด: file:///test.html
→ เชื่อมต่อ: ws://localhost:3000/ws
```

### **Production (หลัง deploy):**
```
เปิด: https://pkderiv.shop/test.html
→ เชื่อมต่อ: wss://pkderiv.shop/ws