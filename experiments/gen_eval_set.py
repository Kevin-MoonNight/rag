#!/usr/bin/env python3
"""產生比較大的檢索評測集：很多份版面與句型幾乎相同、只有主題和數字不同的文件（在 rag-ref 容器內執行）。

目的是測「相似文件很多時，圖片向量還分不分得出來」。6 類文件，每類 10 個主題，共 60 份、120 個查詢。
同一類的 10 份文件句型完全相同，只差主題名稱和幾個數字，所以查詢要靠主題名稱才找得到正確那一份。

輸出到 /rag/testdata/eval60/：
  texts.json               docs（title、text、lang、family）與 queries（text、doc）
  pages/page_XXX.png       乾淨的頁面圖（A4，150 dpi）
  pages/page_XXX_scan.jpg  模擬掃描（輕微旋轉、模糊、雜點、JPEG 壓縮）

內容全部是自己編的，不含任何真實資料。用法：python gen_eval_set.py
"""
import json
import os
import random
import sys

sys.path.insert(0, "/rag/experiments")
from gen_testdata import render_page, scan_like  # noqa: E402

OUT = "/rag/testdata/eval60"
ZH_DIGITS = "零一二三四五六七八九"


def zh(n: int) -> str:
    """1 到 99 的中文數字。"""
    if n < 10:
        return ZH_DIGITS[n]
    tens, ones = divmod(n, 10)
    return ("" if tens == 1 else ZH_DIGITS[tens]) + "十" + (ZH_DIGITS[ones] if ones else "")


def loan(s: str, r: random.Random) -> dict:
    a, b = r.choice([100, 200, 300, 500, 800]), r.choice([1000, 1500, 2000, 3000, 5000])
    return {
        "title": f"{s}審查作業要點",
        "lang": "zh",
        "body": [
            f"第一條　本要點適用於全行各營業單位受理之{s}案件。",
            f"第二條　單筆{s}金額超過新臺幣{a}萬元者，須經{zh(r.choice([2, 3]))}位授信審查人員分別審查，並由審查部主管覆核後，始得核准。",
            f"第三條　{s}金額超過新臺幣{b}萬元者，除前條程序外，應提報授信審議委員會審議。",
            f"第四條　申請人應檢附最近{zh(r.choice([3, 6, 12]))}個月之收入證明；未能提供時，應於審查意見中敘明理由。",
            f"第五條　{s}之擔保品鑑價報告有效期限為{zh(r.choice([3, 4, 6, 9, 12]))}個月，逾期應重新辦理鑑價。",
        ],
        "queries": [f"{s}超過多少金額需要提報授信審議委員會？", f"{s}的擔保品鑑價報告有效期限多久？"],
    }


def retention(s: str, r: random.Random) -> dict:
    dept = r.choice(["資訊處", "法令遵循處", "營運管理處", "稽核處", "客戶服務部", "風險管理處", "總務處"])
    return {
        "title": f"{s}保存年限規定",
        "lang": "zh",
        "body": [
            f"一、{s}自產生之日起至少保存{zh(r.choice([1, 2, 3, 5, 7, 10, 15]))}年。",
            f"二、涉及疑似洗錢或訴訟案件者，{s}應延長保存至案件終結後{zh(r.choice([2, 3, 5]))}年。",
            f"三、{s}之保管單位為{dept}，調閱須經該單位主管核准。",
            f"四、保存期間屆滿後，應依資料銷毀作業程序辦理銷毀，並留存銷毀紀錄{zh(r.choice([1, 3, 5]))}年。",
            "五、主管機關或司法機關來函要求延長保存者，從其規定。",
        ],
        "queries": [f"{s}要保存幾年？", f"{s}由哪個單位保管？"],
    }


def ward(s: str, r: random.Random) -> dict:
    return {
        "title": f"{s}病房用藥核對流程",
        "lang": "zh",
        "body": [
            f"病人入住{s}病房後，護理人員須於{zh(r.choice([4, 8, 12, 24, 48]))}小時內完成用藥核對。",
            "核對內容包括病人入院前使用之處方藥、成藥、中藥及保健食品。",
            f"如發現入院醫囑與原用藥有差異，應於{zh(r.choice([15, 30, 60]))}分鐘內通報主治醫師，並於病歷記載處理結果。",
            f"{s}病房之高警訊藥品須由{zh(r.choice([2, 3]))}位護理人員共同核對後始得給藥，並每{zh(r.choice([4, 8, 12, 24]))}小時清點一次。",
            "出院前應再次核對出院帶藥，並向病人或家屬說明用法與注意事項。",
        ],
        "queries": [f"{s}病房的病人入院後多久內要完成用藥核對？", f"{s}病房的高警訊藥品多久清點一次？"],
    }


def clinic(s: str, r: random.Random) -> dict:
    return {
        "title": f"{s}門診掛號及退費辦法",
        "lang": "zh",
        "body": [
            f"一、{s}初診病人請攜帶健保卡及身分證件至{zh(r.choice([1, 2, 3, 5]))}樓掛號櫃檯辦理。",
            f"二、{s}網路預約掛號開放時間為看診日前{zh(r.choice([7, 14, 21, 28, 30, 60]))}天至看診當日上午八時。",
            "三、已完成掛號但未看診者，得於當日門診結束前至櫃檯辦理退掛，掛號費全額退還。",
            f"四、已看診但未領藥者，藥費部分得於{zh(r.choice([3, 7, 10, 14]))}日內憑收據辦理退費。",
            f"五、{s}每診次掛號人數上限為{zh(r.choice([30, 40, 50, 60, 80]))}人，額滿後不再受理現場掛號。",
        ],
        "queries": [f"{s}網路掛號最早可以提前幾天預約？", f"{s}每診最多可以掛幾個人？"],
    }


def incident(s: tuple[str, str], r: random.Random) -> dict:
    en, zh_name = s
    return {
        "title": f"Incident Response Procedure: {en.title()}",
        "lang": "en",
        "body": [
            f"1. Any employee who detects a suspected incident affecting the {en} must report it to the Security Operations Center within {r.choice([10, 15, 30, 45, 60])} minutes.",
            f"2. Incidents are classified into four severity levels. Level 1 incidents involve confirmed leakage of data from the {en}.",
            f"3. For Level 1 incidents, the Chief Information Security Officer must be notified within {r.choice([1, 2, 4])} hours and the regulator within {r.choice([12, 24, 48, 72])} hours.",
            f"4. The {en} must be isolated from the network before any forensic image is taken.",
            f"5. A post-incident review report must be completed within {r.choice([5, 10, 15, 20])} business days and retained for {r.choice([3, 5, 7, 10])} years.",
        ],
        "queries": [
            f"How quickly must an incident in the {en} be reported to the Security Operations Center?",
            f"{zh_name}發生資料外洩時要在幾小時內通報主管機關？",
        ],
    }


def access(s: tuple[str, str], r: random.Random) -> dict:
    en, zh_name = s
    approver = r.choice(["the department head", "the Chief Information Security Officer", "the compliance officer", "the branch manager", "the medical director"])
    return {
        "title": f"Remote Access Policy for {en.title()}",
        "lang": "en",
        "body": [
            f"Remote access for {en} is permitted only through the corporate VPN with multi-factor authentication.",
            f"{en.capitalize()} must not store customer or patient data on personal devices. Company laptops are encrypted with full-disk encryption.",
            f"VPN sessions for {en} are automatically disconnected after {r.choice([5, 10, 15, 20, 30])} minutes of inactivity and have a maximum duration of {r.choice([4, 8, 10, 12])} hours.",
            f"Access from outside the country requires prior written approval from {approver}.",
            f"Access rights for {en} are reviewed every {r.choice([3, 6, 12])} months.",
        ],
        "queries": [
            f"How long before an idle VPN session is disconnected for {en}?",
            f"{zh_name}的遠端存取權限多久檢視一次？",
        ],
    }


FAMILIES = [
    ("loan", loan, ["房屋貸款", "汽車貸款", "信用貸款", "企業週轉金貸款", "就學貸款", "農業貸款", "中小企業貸款", "留學貸款", "房屋修繕貸款", "青年創業貸款"]),
    ("retention", retention, ["存款交易紀錄", "開戶申請資料", "營業廳監視錄影", "客服電話錄音", "客戶申訴紀錄", "內部稽核報告", "授信案件卷宗", "自動櫃員機交易日誌", "網路銀行登入紀錄", "外匯交易憑證"]),
    ("ward", ward, ["心臟內科", "胸腔內科", "腎臟科", "小兒科", "婦產科", "骨科", "神經內科", "血液腫瘤科", "加護", "精神科"]),
    ("clinic", clinic, ["家醫科", "眼科", "耳鼻喉科", "皮膚科", "牙科", "復健科", "泌尿科", "中醫科", "身心科", "整形外科"]),
    ("incident", incident, [
        ("core banking system", "核心銀行系統"), ("ATM network", "自動櫃員機網路"), ("online banking portal", "網路銀行"),
        ("corporate email system", "公司電子郵件系統"), ("hospital information system", "醫院資訊系統"),
        ("medical imaging archive", "醫學影像儲傳系統"), ("laboratory information system", "檢驗資訊系統"),
        ("payment card system", "信用卡系統"), ("mobile banking app", "行動銀行"), ("data warehouse", "資料倉儲")]),
    ("access", access, [
        ("branch tellers", "分行櫃員"), ("software developers", "軟體開發人員"), ("call center agents", "客服中心人員"),
        ("external auditors", "外部稽核人員"), ("physicians", "醫師"), ("nursing staff", "護理人員"),
        ("IT administrators", "系統管理員"), ("contractors", "外包廠商人員"), ("executives", "高階主管"),
        ("research staff", "研究人員")]),
]


def main() -> None:
    os.makedirs(f"{OUT}/pages", exist_ok=True)
    rnd = random.Random(20261013)
    docs, queries = [], []
    for family, make, subjects in FAMILIES:
        for subject in subjects:
            doc = make(subject, rnd)
            idx = len(docs)
            page = f"{OUT}/pages/page_{idx:03d}.png"
            render_page(doc, page)
            scan_like(page, f"{OUT}/pages/page_{idx:03d}_scan.jpg", seed=1000 + idx)
            docs.append({"title": doc["title"], "text": doc["title"] + "\n" + "\n".join(doc["body"]), "lang": doc["lang"], "family": family})
            for q in doc["queries"]:
                queries.append({"text": q, "doc": idx})
    with open(f"{OUT}/texts.json", "w", encoding="utf-8") as f:
        json.dump({"docs": docs, "queries": queries}, f, ensure_ascii=False, indent=1)
    print(f"wrote {len(docs)} docs, {len(queries)} queries, {2 * len(docs)} page images to {OUT}")


if __name__ == "__main__":
    main()
