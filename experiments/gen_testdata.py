#!/usr/bin/env python3
"""產生驗證用的測試資料（在 rag-ref 容器內執行，需要 Pillow 和 Noto CJK 字型）。

輸出到 /rag/testdata/：
  texts.json            查詢與文件的文字
  pages/page_XX.png     把文件內容畫成 A4 頁面的圖片（模擬掃描頁，150 dpi）
  pages/page_XX_scan.jpg  同一頁加上輕微旋轉、雜訊、JPEG 壓縮（比較接近真的掃描）

內容是自己編的銀行 / 醫院內規風格文字，不含任何真實資料。
"""
import json
import os
import random

from PIL import Image, ImageDraw, ImageFilter, ImageFont

OUT = "/rag/testdata"
FONT = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"
FONT_BOLD = "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc"
TC_INDEX = 3  # NotoSansCJK ttc 內的順序：0 JP, 1 KR, 2 SC, 3 TC, 4 HK

DOCS = [
    {
        "title": "授信審查作業要點",
        "lang": "zh",
        "body": [
            "第一條　本要點依據本行授信政策訂定，適用於全行各營業單位之企業及個人授信案件。",
            "第二條　單筆授信金額超過新臺幣五百萬元者，須經兩位授信審查人員分別審查，並由審查部主管覆核後，始得核准。",
            "第三條　授信金額超過新臺幣三千萬元者，除前條程序外，應提報授信審議委員會審議。",
            "第四條　借款人之財務報表應為最近一年度經會計師查核簽證者；未能提供時，應於審查意見中敘明理由。",
            "第五條　擔保品之鑑價報告有效期限為六個月，逾期應重新辦理鑑價。",
        ],
    },
    {
        "title": "交易紀錄保存年限規定",
        "lang": "zh",
        "body": [
            "一、法人帳戶之交易紀錄，自帳戶結清之日起保存七年。",
            "二、個人帳戶之交易紀錄，自帳戶結清之日起保存五年。",
            "三、疑似洗錢交易之申報紀錄及相關佐證資料，應保存十年。",
            "四、保存期間屆滿後，應由資訊部門依資料銷毀作業程序辦理銷毀，並留存銷毀紀錄三年。",
            "五、主管機關或司法機關來函要求延長保存者，從其規定。",
        ],
    },
    {
        "title": "心臟內科病房用藥核對流程",
        "lang": "zh",
        "body": [
            "病人入住心臟內科病房後，護理人員須於二十四小時內完成用藥核對。",
            "核對內容包括病人入院前使用之處方藥、成藥、中藥及保健食品。",
            "如發現入院醫囑與原用藥有差異，應立即通報主治醫師，並於病歷記載處理結果。",
            "使用抗凝血劑之病人，每日應檢視凝血功能檢驗數值，INR 超過 3.5 時暫停給藥並通知醫師。",
            "出院前應再次核對出院帶藥，並向病人或家屬說明用法與注意事項。",
        ],
    },
    {
        "title": "Information Security Incident Response Procedure",
        "lang": "en",
        "body": [
            "1. Any employee who detects a suspected security incident must report it to the Security Operations Center within 30 minutes.",
            "2. Incidents are classified into four severity levels. Level 1 incidents involve confirmed leakage of customer data.",
            "3. For Level 1 incidents, the Chief Information Security Officer must be notified within one hour and the regulator within 24 hours.",
            "4. Affected systems must be isolated from the network before any forensic image is taken.",
            "5. A post-incident review report must be completed within 10 business days and retained for five years.",
        ],
    },
    {
        "title": "門診掛號及退費辦法",
        "lang": "zh",
        "body": [
            "一、初診病人請攜帶健保卡及身分證件至一樓掛號櫃檯辦理。",
            "二、網路預約掛號開放時間為看診日前二十八天至看診當日上午八時。",
            "三、已完成掛號但未看診者，得於當日門診結束前至櫃檯辦理退掛，掛號費全額退還。",
            "四、已看診但未領藥者，藥費部分得於七日內憑收據辦理退費。",
            "五、每位病人同一日同一科別以掛號一次為限。",
        ],
    },
    {
        "title": "Employee Remote Access Policy",
        "lang": "en",
        "body": [
            "Remote access to the internal network is permitted only through the corporate VPN with multi-factor authentication.",
            "Personal devices must not be used to store customer data. Company laptops are encrypted with full-disk encryption.",
            "VPN sessions are automatically disconnected after 15 minutes of inactivity and have a maximum duration of 10 hours.",
            "Access from outside the country requires prior written approval from the department head and the security team.",
            "Violations of this policy may result in disciplinary action, up to and including termination of employment.",
        ],
    },
]

# 每個查詢標明正確答案是哪一份文件（DOCS 的索引）
QUERIES = [
    {"text": "貸款超過五百萬元需要幾位授信人員審查？", "doc": 0},
    {"text": "擔保品鑑價報告多久會過期？", "doc": 0},
    {"text": "法人帳戶的交易紀錄要保存幾年？", "doc": 1},
    {"text": "疑似洗錢交易的申報資料保存期限", "doc": 1},
    {"text": "病人住院後多久內要完成用藥核對？", "doc": 2},
    {"text": "INR 數值過高時抗凝血劑該怎麼處理？", "doc": 2},
    {"text": "How quickly must a suspected security incident be reported?", "doc": 3},
    {"text": "客戶資料外洩要在多久內通報主管機關？", "doc": 3},  # 中文問、英文文件
    {"text": "掛號後沒看診可以退費嗎？", "doc": 4},
    {"text": "網路掛號最早可以提前幾天預約？", "doc": 4},
    {"text": "How long before an idle VPN session is disconnected?", "doc": 5},
    {"text": "從國外連回公司網路需要誰核准？", "doc": 5},  # 中文問、英文文件
]


def wrap(draw: ImageDraw.ImageDraw, text: str, font: ImageFont.FreeTypeFont, max_w: int) -> list[str]:
    lines, cur = [], ""
    # 英文依單字斷行，中文依字元斷行
    units = text.split(" ") if all(ord(c) < 0x2E80 for c in text) else list(text)
    sep = " " if units and len(units[0]) > 1 and all(ord(c) < 0x2E80 for c in text) else ""
    for u in units:
        trial = (cur + sep + u) if cur else u
        if draw.textlength(trial, font=font) <= max_w:
            cur = trial
        else:
            if cur:
                lines.append(cur)
            cur = u
    if cur:
        lines.append(cur)
    return lines


def render_page(doc: dict, path: str) -> None:
    w, h = 1240, 1754  # A4 @ 150 dpi
    img = Image.new("RGB", (w, h), "white")
    d = ImageDraw.Draw(img)
    f_title = ImageFont.truetype(FONT_BOLD, 44, index=TC_INDEX)
    f_body = ImageFont.truetype(FONT, 30, index=TC_INDEX)
    f_small = ImageFont.truetype(FONT, 20, index=TC_INDEX)
    margin = 110
    y = 130
    d.text((margin, y), doc["title"], font=f_title, fill="black")
    y += 80
    d.line((margin, y, w - margin, y), fill="black", width=2)
    y += 40
    for para in doc["body"]:
        for line in wrap(d, para, f_body, w - 2 * margin):
            d.text((margin, y), line, font=f_body, fill="black")
            y += 46
        y += 22
    d.text((margin, h - 110), "內部文件　請勿外流" if doc["lang"] == "zh" else "Internal use only", font=f_small, fill=(90, 90, 90))
    d.text((w - margin - 80, h - 110), "第 1 頁" if doc["lang"] == "zh" else "Page 1", font=f_small, fill=(90, 90, 90))
    img.save(path)


def scan_like(src: str, dst: str, seed: int) -> None:
    rnd = random.Random(seed)
    img = Image.open(src).convert("L")
    img = img.rotate(rnd.uniform(-1.2, 1.2), expand=False, fillcolor=255, resample=Image.BICUBIC)
    img = img.filter(ImageFilter.GaussianBlur(0.6))
    px = img.load()
    for _ in range(img.width * img.height // 400):  # 少量雜點
        x, y = rnd.randrange(img.width), rnd.randrange(img.height)
        px[x, y] = rnd.randrange(0, 160)
    img = img.point(lambda v: min(255, int(v * 0.93 + 12)))  # 紙張略灰
    img.convert("RGB").save(dst, quality=60)


def main() -> None:
    os.makedirs(f"{OUT}/pages", exist_ok=True)
    for i, doc in enumerate(DOCS):
        p = f"{OUT}/pages/page_{i:02d}.png"
        render_page(doc, p)
        scan_like(p, f"{OUT}/pages/page_{i:02d}_scan.jpg", seed=i)
    with open(f"{OUT}/texts.json", "w", encoding="utf-8") as f:
        json.dump(
            {
                "docs": [{"title": d["title"], "text": d["title"] + "\n" + "\n".join(d["body"])} for d in DOCS],
                "queries": QUERIES,
            },
            f,
            ensure_ascii=False,
            indent=1,
        )
    print(f"wrote {len(DOCS)} docs, {len(QUERIES)} queries, {2*len(DOCS)} page images to {OUT}")


if __name__ == "__main__":
    main()
