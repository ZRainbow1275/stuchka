#!/usr/bin/env python3
"""Deterministically generate the pii-200 manifest (ai/05 §5.7 / ai/04 §4.7).

Ground truth is GENUINE, never fabricated:
  - Positives embed a real-format token that the PRODUCTION detector must catch: phone numbers
    matching the regex `(?:\\+?86)?(1[3-9][0-9]{9})`, the two ISO-7064-mod-11-2-VALID id cards and
    the four Luhn-VALID bank cards taken from hsd/tests/regex_unit.rs, and audio/medical file paths
    matching the path patterns. Each positive's known kind is the label.
  - Negatives are benign text or near-miss tokens the validators MUST reject (checksum-failing id
    cards / bank cards, non-1[3-9] phone-like digit runs, paths without the trigger keyword).

The detector independently validates every token (regex + ISO-7064 + Luhn), so recall measures real
detection and fp_rate measures real false positives. Run: `python generate.py` -> writes manifest.json.
"""
import json
import os

# 50 genuine phone numbers: each is "1" + a 2nd digit in 3..9 + 9 more digits = matches the regex.
PHONES = [
    "13800138000", "13911112222", "13533224455", "13688889999", "13712345678",
    "15011112222", "15122223333", "15233224455", "15912345678", "15088887777",
    "17012345678", "17612345678", "17712345678", "17812345678", "17012348888",
    "18612345678", "18899990000", "18112345678", "18212345678", "18888887777",
    "19912345678", "19112345678", "19212345678", "19812345678", "19012348888",
    "13312345678", "13412345678", "13512345678", "13612345678", "13912348888",
    "14712345678", "14512678901", "16612345678", "16512345678", "16612348888",
    "13066778899", "13166224488", "13266338855", "15366440099", "15566551100",
    "17066220033", "18066330044", "19066440055", "13977665544", "15988776655",
    "17099887766", "18011223344", "19022334455", "13755667788", "15844556677",
]
# Both pass ISO 7064 mod 11-2 (from hsd/tests/regex_unit.rs positive set).
IDCARDS = ["110101199001010074", "11010119900101004X"]
# All pass Luhn (from the same fixture set).
BANKCARDS = ["6222021234567894", "6225881234567890120", "4000001234567899", "5100001234567895"]

AUDIO_DIRS = ["D:/录音", "~/录音", "C:\\录音", "../通话", "D:/录像"]
AUDIO_KEY = ["工资沟通", "与张经理通话", "加班录像", "老板承诺", "调岗谈话"]
AUDIO_EXT = ["m4a", "mp3", "wav", "amr", "aac"]
MED_DIRS = ["D:/病历", "~/诊断书", "C:\\出院", "../病历", "D:/工伤鉴定"]
MED_KEY = ["住院记录", "工伤鉴定", "诊断书", "出院小结", "检查报告"]
MED_EXT = ["pdf", "jpg", "jpeg", "png", "docx"]

PHONE_CTX = [
    "我老板的电话是{},他从来不接我。", "联系方式{}请尽快回复。", "号码:{}是工头留的。",
    "请拨打{}核实工资发放情况。", "他给我的手机号{}已停机。", "{}是公司财务的电话。",
    "我的联系电话{},方便随时沟通。", "工友说包工头电话{}。", "紧急联系人{}。", "{}",
]
ID_CTX = ["身份证号{}请核对。", "我的身份证是{}。", "对方身份证{}已登记。",
          "申请人身份证号码:{}。", "证件号{}用于实名。"]
BANK_CTX = ["工资卡号{}打款。", "我的银行卡是{}。", "请汇至{}。",
            "对公账户{}收款。", "卡号{}尾号核对。"]

# Negatives: near-miss tokens the validators MUST reject + benign text.
NEAR_MISS = [
    "12345678901", "1380013800", "138001380000", "10086", "请拨打95588咨询",
    "11111111111", "12012345678", "02012345678", "1234567890", "100861234567",
    "110101199001010070", "110101199013010074", "110101199001320074",
    "010101199001010074", "11010119900101007",
    "卡号1234567890123456是订单号", "6222021234567890打款失败", "8888888888888888",
    "9999999999999999", "0000000000000000",
]
BENIGN = [
    "今天天气很好我们一起去工地。", "项目进度基本正常请放心。", "请提交本季度的工作总结。",
    "明天下午三点在会议室开会。", "合同金额大约是五万元整。", "这个月加班比较多很辛苦。",
    "工资条上写得清清楚楚。", "我们公司在工业园区上班。", "师傅说这批货周五交付。",
    "他答应下周把钱结清。", "劳动法保护每一位劳动者。", "记得带好身份证原件复印件。",
    "食堂的午饭味道还不错。", "宿舍楼下有便利店很方便。", "上个月的考勤记录已经核对。",
    "包工头姓王为人还算实在。", "这份协议需要双方签字盖章。", "仲裁委员会在市中心办公。",
    "希望尽快拿到属于自己的报酬。", "工地的安全帽一定要戴好。", "今年的项目计划已经排满。",
    "厂里新来了一批年轻工人。", "周末打算回老家看看父母。", "技术培训安排在下周二进行。",
    "材料采购清单已经发给采购部。", "车间主任今天请假了。", "这台机器需要定期保养。",
    "工会organize了一次活动。", "午休时间是十二点到一点。", "项目验收标准非常严格。",
    "他在车间工作了整整十年。", "新员工入职要签劳动合同。", "安全生产人人有责。",
    "这个季度的奖金还没发。", "仓库的库存需要盘点一次。", "下个月要去外地出差。",
    "公司食堂的饭菜挺便宜。", "他负责整条生产线的质检。", "工资应当按月足额发放。",
    "请假需要提前向班组长报备。", "维修单已经提交给后勤。", "今天的产量超过了计划。",
    "这批订单的交期比较紧。", "车间温度有点高要注意防暑。", "他是去年评上的先进个人。",
    "考勤打卡机又出故障了。", "项目组今晚要赶进度。", "仓库管理员负责出入库登记。",
    "工地周边有几家小饭馆。", "新规定要求佩戴工牌上岗。", "他打算申请劳动仲裁。",
    "这份证据材料要整理归档。", "厂区门口有保安值班。", "生产任务完成得不错。",
    "他对计算结果有些疑问。", "请把材料交到办公室前台。", "今天的早会取消了。",
    "设备故障已经报修处理。", "这个岗位需要持证上岗。", "下班后记得关好门窗。",
]
BENIGN_PATHS = [
    "D:/文档/合同.pdf", "D:/音乐/歌曲.mp3", "C:\\下载\\说明书.pdf", "~/图片/风景.png",
    "D:/资料/报表.docx", "录音很重要但这只是一句话", "D:/视频/教程.mp4",
    "C:\\备份\\数据.zip", "~/桌面/笔记.txt", "D:/项目/源码.rs",
    "病历是医疗文件的统称", "D:/工作/计划.xlsx",
]


def positives():
    cases = []
    for i, p in enumerate(PHONES):
        ctx = PHONE_CTX[i % len(PHONE_CTX)]
        cases.append(("pos_phone_%02d" % i, ctx.format(p), "phone"))
    for i in range(20):
        path = "%s/%s.%s" % (AUDIO_DIRS[i % len(AUDIO_DIRS)], AUDIO_KEY[i % len(AUDIO_KEY)], AUDIO_EXT[i % len(AUDIO_EXT)])
        cases.append(("pos_audio_%02d" % i, "证据文件位于 %s 请妥善保存。" % path, "audio_path"))
    for i in range(15):
        path = "%s/%s.%s" % (MED_DIRS[i % len(MED_DIRS)], MED_KEY[i % len(MED_KEY)], MED_EXT[i % len(MED_EXT)])
        cases.append(("pos_medical_%02d" % i, "请上传 %s 作为工伤证明。" % path, "medical_record"))
    for i in range(8):
        idc = IDCARDS[i % len(IDCARDS)]
        cases.append(("pos_idcard_%02d" % i, ID_CTX[i % len(ID_CTX)].format(idc), "id_card"))
    for i in range(7):
        bc = BANKCARDS[i % len(BANKCARDS)]
        cases.append(("pos_bankcard_%02d" % i, BANK_CTX[i % len(BANK_CTX)].format(bc), "bank_card"))
    out = []
    for cid, text, kind in cases:
        out.append({
            "id": cid, "text": text, "label": "positive",
            "expect_kinds": [kind], "expect_high_sensitive": True,
            "expect_route_hint": "force_local",
        })
    return out


def negatives():
    out = []
    for i, t in enumerate(NEAR_MISS):
        out.append({"id": "neg_nearmiss_%02d" % i, "text": t, "label": "negative",
                    "expect_kinds": [], "expect_high_sensitive": False})
    benign = BENIGN + BENIGN_PATHS
    need = 100 - len(NEAR_MISS)
    for i in range(need):
        out.append({"id": "neg_benign_%02d" % i, "text": benign[i % len(benign)], "label": "negative",
                    "expect_kinds": [], "expect_high_sensitive": False})
    return out


def main():
    pos = positives()
    neg = negatives()
    cases = pos + neg
    manifest = {
        "dataset": "pii-200",
        "spec": "0529/spec/ai/05-evaluation-set.md §5.7 + ai/04 §4.7",
        "threshold": "recall >= 0.95 AND fp_rate <= 0.05",
        "ground_truth": "Positives embed real-format tokens the production detector must catch (regex-valid phones, ISO-7064-valid id cards, Luhn-valid bank cards, keyword+ext paths); negatives are benign text or checksum-failing near-misses the validators must reject. Generated by generate.py (deterministic, no fabricated answers).",
        "counts": {"positives": len(pos), "negatives": len(neg), "total": len(cases)},
        "cases": cases,
    }
    path = os.path.join(os.path.dirname(__file__), "manifest.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=0)
    print("wrote %s (%d positives + %d negatives = %d)" % (path, len(pos), len(neg), len(cases)))


if __name__ == "__main__":
    main()
