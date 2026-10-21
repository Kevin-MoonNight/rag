#!/usr/bin/env python3
"""產生接近真實文件密度的測試頁（12 pt 字、每頁約 900 字）。

輸出到 /rag/testdata/dense/：texts.json、pages/page_XX.png、pages/page_XX_scan.jpg
內容是自己編的銀行 / 醫院內規風格文字，不含任何真實資料。刻意放入猜不到的數字、代號與專有名詞。
"""
import json
import os
import sys

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, "/rag/experiments")
from gen_testdata import FONT, FONT_BOLD, TC_INDEX, scan_like, wrap  # noqa: E402

OUT = "/rag/testdata/dense"

DOCS = [
    {
        "title": "消費性貸款逾期催收作業細則",
        "body": [
            "第一條　為規範本行消費性貸款逾期案件之催收程序，確保債權並兼顧客戶權益，依據逾期放款管理辦法第十四條訂定本細則。",
            "第二條　借款人未依約繳付本金或利息達七日者，由原承辦分行以簡訊及電話通知；達三十日者，應寄發催告函，並於授信管理系統登錄催收紀錄代碼 C2。",
            "第三條　逾期達六十日仍未清償者，承辦人員應於五個營業日內填具逾期案件報告表，載明逾期原因、擔保品現況及保證人聯絡情形，陳報分行經理核閱後移送區域債權管理中心。",
            "第四條　區域債權管理中心受理後，應於十個營業日內完成借款人及保證人之財產查調，查調項目包括不動產、存款、薪資所得及有價證券，查調費用每件以新臺幣四百五十元為上限。",
            "第五條　借款人提出協商還款申請時，得依其還款能力核予展延，展延期間最長不得超過八十四個月，展延期間之利率不得低於原契約利率減一點二五個百分點。",
            "第六條　單一借款人協商後之月付金低於新臺幣三千元，或展延後之總期數超過一百二十期者，應提報總行債權審議小組核定，不得由區域中心逕行核准。",
            "第七條　催收人員進行電話催收，應於上午八時至下午九時之間為之，每日對同一債務人之撥打次數不得超過三次，且不得向債務人以外之第三人透露債務內容。",
            "第八條　委外催收應以經主管機關備查之機構為限，委外期間每案最長十二個月，受委託機構每月五日前應提交催收進度報告，本行得隨時抽查其通話錄音。",
            "第九條　逾期達一百八十日且經評估無法收回者，得依規定轉銷呆帳；轉銷金額在新臺幣二百萬元以下者由總經理核定，超過者應提報董事會。",
            "第十條　呆帳轉銷後仍應持續追索，每十二個月至少重新查調債務人財產一次，並將結果登錄於債權管理系統，保存期限為轉銷日起十五年。",
            "第十一條　本細則所定各項期限遇例假日順延，所稱營業日不包括週六補行上班日。",
            "第十二條　本細則經風險管理委員會通過後施行，修正時亦同。",
        ],
    },
    {
        "title": "加護病房高警訊藥品給藥作業規範",
        "body": [
            "一、目的：為降低高警訊藥品之給藥錯誤，保障重症病人安全，特訂定本規範，適用於內科、外科及心臟血管加護病房。",
            "二、本規範所稱高警訊藥品，指氯化鉀注射液、胰島素、肝素、嗎啡類止痛劑、升壓劑及神經肌肉阻斷劑等六類，品項清單由藥事委員會每年十一月檢討公告。",
            "三、濃度百分之十五之氯化鉀注射液不得存放於病房常備藥櫃，須由藥劑部調配為每一百毫升含二十毫當量之稀釋液後始得發出；周邊靜脈輸注速率每小時不得超過十毫當量。",
            "四、胰島素連續輸注應使用專用輸液幫浦，起始劑量由醫師依體重開立，護理人員每小時監測血糖一次，血糖值低於每分升七十毫克時應立即停止輸注並通知值班醫師。",
            "五、肝素輸注期間，每六小時應檢驗活化部分凝血活酶時間，目標範圍為對照值之一點五至二點五倍；超過三倍時暫停輸注一小時後依醫囑調降劑量。",
            "六、給藥前應由兩名護理人員獨立核對病人身分、藥名、劑量、濃度、途徑及輸注速率，核對人員均應於電子給藥紀錄簽章，不得由同一人代為簽核。",
            "七、升壓劑應經中心靜脈導管給予，更換輸液袋時採雙幫浦接續方式，新舊輸液重疊時間不得少於五分鐘，以避免血壓驟降。",
            "八、神經肌肉阻斷劑應存放於貼有紅色警示標籤之獨立藥盒，使用期間病人須全程接受呼吸器支持，並每四小時以四連刺激評估阻斷深度。",
            "九、發生給藥異常事件時，當班護理人員應於三十分鐘內口頭通報護理長，並於二十四小時內完成病人安全通報系統之線上通報，事件等級由品質管理中心判定。",
            "十、藥劑部每月抽查各加護病房高警訊藥品之存放與標示，抽查結果低於九十分之單位應於十四日內提出改善計畫。",
            "十一、新進護理人員須完成四小時高警訊藥品訓練課程並通過測驗，成績達八十五分以上始得獨立執行本規範所列藥品之給藥作業。",
        ],
    },
]


def render_dense(doc: dict, path: str) -> None:
    w, h = 1240, 1754  # A4 @ 150 dpi
    img = Image.new("RGB", (w, h), "white")
    d = ImageDraw.Draw(img)
    f_title = ImageFont.truetype(FONT_BOLD, 34, index=TC_INDEX)
    f_body = ImageFont.truetype(FONT, 25, index=TC_INDEX)  # 25 px @ 150 dpi = 12 pt
    f_small = ImageFont.truetype(FONT, 18, index=TC_INDEX)
    margin = 120
    y = 110
    d.text((margin, y), doc["title"], font=f_title, fill="black")
    y += 64
    d.line((margin, y, w - margin, y), fill="black", width=2)
    y += 28
    for para in doc["body"]:
        for line in wrap(d, para, f_body, w - 2 * margin):
            d.text((margin, y), line, font=f_body, fill="black")
            y += 37
        y += 12
    assert y < h - 130, f"page overflow: y={y}"
    d.text((margin, h - 100), "內部文件　請勿外流", font=f_small, fill=(90, 90, 90))
    d.text((w - margin - 70, h - 100), "第 1 頁", font=f_small, fill=(90, 90, 90))
    img.save(path)


def main() -> None:
    os.makedirs(f"{OUT}/pages", exist_ok=True)
    for i, doc in enumerate(DOCS):
        p = f"{OUT}/pages/page_{i:02d}.png"
        render_dense(doc, p)
        scan_like(p, f"{OUT}/pages/page_{i:02d}_scan.jpg", seed=100 + i)
        print(f"page {i}: {sum(len(s) for s in doc['body']) + len(doc['title'])} chars")
    with open(f"{OUT}/texts.json", "w", encoding="utf-8") as f:
        json.dump({"docs": [{"title": d["title"], "text": d["title"] + "\n" + "\n".join(d["body"])} for d in DOCS], "queries": []}, f, ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
