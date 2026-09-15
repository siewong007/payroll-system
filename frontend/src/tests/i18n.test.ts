import { describe, expect, it } from 'vitest';
import en from '@/i18n/locales/en';
import ms from '@/i18n/locales/ms';
import zhCN from '@/i18n/locales/zh-CN';
import zhTW from '@/i18n/locales/zh-TW';

// Flatten { a: { b: 'x' } } to { 'a.b': 'x' }.
function flat(obj: Record<string, unknown>, prefix = '', out: Record<string, string> = {}) {
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object') flat(v as Record<string, unknown>, key, out);
    else out[key] = v as string;
  }
  return out;
}

const LOCALES: Record<string, Record<string, string>> = {
  en: flat(en as unknown as Record<string, unknown>),
  ms: flat(ms as unknown as Record<string, unknown>),
  'zh-CN': flat(zhCN as unknown as Record<string, unknown>),
  'zh-TW': flat(zhTW as unknown as Record<string, unknown>),
};
const EN = LOCALES.en;

// Keys whose values are legitimately identical to en: locale autonyms, statutory
// acronyms, brand names, loanwords, and interpolation-only templates.
const SAME_AS_EN = new Set([
  'common.editName', 'common.edit', 'common.import', 'common.status', 'common.id',
  'nav.portalBadge',
  'language.en', 'language.ms', 'language.zh-CN', 'language.zh-TW',
  'time.minutesShort',
  'enums.attendanceMethod.face_id', 'enums.attendanceMethod.manual',
  'employees.fields.nric', 'employees.fields.bank', 'employees.fields.tin',
  'employees.columns.status',
  'attendance.geofence.title', 'attendance.kiosk.secondsShort',
  'attendance.checkin.faceId', 'attendance.kioskModal.cols.label',
  'payroll.period', 'payroll.fields.pcb', 'payroll.fields.zakat', 'payroll.cols.ot',
  'payroll.detail.cancelledReason', 'payroll.detail.editPcb',
  'payroll.detail.journalCols.debit', 'payroll.detail.basisCols.domain',
  'portal.teamCalendar.leaveTitle',
  'portal.payslips.pcbMtd', 'portal.payslips.tabungHaji',
  'portal.payslips.ytdPcb', 'portal.payslips.ytdZakat',
  'companies.phonePlaceholder', 'companies.emailPlaceholder',
  'attendanceSettings.faceIdTitle', 'approvals.summaryTitle',
  'reports.cols.zakat', 'teams.tag', 'settings.units.min',
]);

// language.* autonyms intentionally carry each locale's own script
// ("简体中文" under zh-TW is the label OF zh-CN, not a script slip).
const SCRIPT_EXEMPT = (k: string) => k.startsWith('language.');

// Characters that exist ONLY in Simplified Chinese — every entry here has a
// distinct Traditional form (发→發, 会→會, 门→門). Chars that are identical
// in both scripts (需 限 零 餐 峰 圈 横 温 没 涌 毁 杰 栖 斗 筑 虫 虚 腊
// 耻 羡 苹 帘 弃 适 启 嘻 噪 嚎 嚼 嘟 遥 云 几 革 靴 靶 鞋 鞍 鞭 食 里
// 于 台 后) are deliberately excluded so legit zh-TW text cannot false-positive.
const SIMPLIFIED_ONLY = new Set(
  '们这为无与长门问关员来对说时会应进还动样两经点现将处报张认导设计记让话语读课请证谁调论试误责财货购费资账车轮输转载较辆运连迟选递逻边达医双难电龙鸟马鱼风飞饭饮饿馆书画网线终组细绍结给统绝继绩绿绪编缩级纪红约纳纸练罗罚罢联职听声脑脸脚营获蓝虑卫视觉览观规订训议讯许访评识诉诊词译该详诱诸诺谋谎谐谓谜谢谣谦谨历压厌参变号叹吗响唤喊喷嗓嘘嘱嚣团围图坏块坚坛坟坠垄垫墙壮寿尘尝尽层属岁岗岛岭峡币师帐帜带帮广庄庆库庙庞废开异弥弯弹归当录彻径忆忧怀态总恋恳恶恼惊惧惨惩惭惯愤愿懒戏战扑执扩扫扬扰扰抛抢护担拟拢拣拥拦拨择挂挚挠挡挣挤挥捞损捡换捣据掷搁搂搅携摄摆摇摊撑敌敛数斩断旧显晒晓晕暂机杀杂权杆条杨极构枢枣枪枫柜标栈栋栏树档桥桦桨桩梦检楼槛樱橱欢欧歼残殴毕毙气汇汉汤沟沥沦沧沪泻泼泽洁洒浅浆浇浊测济浑浓涂涛涝涟涡涤润涧涨涩渊渔渗湾湿溃溅滚滞满滤滥滨滩潜澜灭灯灵灿炉炼炽烁烂烛烦烧烩烫热焕焖爱爷牵牺状犹独狭狮狱猎猪猫献盐监盖盗盘睁瞒瞩矿码砖砚础硕确碍礼祷祸禄禅离秆种积称秽税稳穷窃窍窑窜窝窥竖竞笃笋笔笼筛筹签简箩箫篓篮篱类粪粮紧纠纤纯纱纲纵纷纹纺纽绊绍绑绒绕绘绚络绞绢绣绥绫绮绳维绵绷绸综绽缀缆缉缎缓缔缕缘缚缝缠缤缨缪缭缴羁耸聋聪肃肠肤肾肿胀胁胆胶脉脏脓腻腾舰舱艰艳艺节苍苏茎茧荆荐荚荡荣荤荧荫药莱莲莹莺萝萤萧萨葱蒋蔷蔼蕴虏虫虽虾蚀蚁蚂蚕蛮蛰蜗蜡蝇蝉蝎衅衔补衬袄袜袭装裤贝贞负贡贤败质贩贪贫贬贮贯贰贱贴贵贷贸贺贻贼贾贿赂赃赊赋赌赎赏赐赔赖赘赚赛赞赠赡赢赵赶趋跃践踊踪躯轧轨轩软轰轴轻辅辈辉辊辍辐辑辕辖辗辙辞辩辫辽迁迈违迹逊遗邮邻郑郸酝酱酿释鉴针钉钓钙钝钞钟钠钢钥钦钩钮钱钳钻铁铃铅铆铜铝铭银铸铺链销锁锄锅锈锋锌锐错锚锡锦键锯锤锥锭锰锹锻镀镇镜镰闲闷闸闹闺闻闽阀阁阂阅阐阔队阳阴阵阶际陆陈陕陨险随隐隶雏虽鸡雳雾霁静靥韧韩韵页顶顷项顺须顾顿颁颂预颅领颇颈颊频颓颖颗题额颚颜颠饥饨饪饯饰饱饲饴饵饶饷饺饼馁馄馅馈馋馍馏馒驭驮驯驰驱驳驴驶驸驹驻驼驾驿骁骂骄骆骇骋验骏骑骗骚骛骡骤骥鱿鲁鲍鲑鲜鲢鲤鲨鲫鲸鳃鳄鳅鳍鳕鳖鳗鳞鸠鸢鸣鸥鸦鸭鸵鸽鸿鹃鹅鹊鹌鹏鹑鹕鹜鹰鹭麦麸齐齿龄龈龋龚龟',
);

// Characters that exist ONLY in Traditional Chinese — a hit in zh-CN means a
// Traditional slip. Same rule: same-in-both chars excluded.
const TRADITIONAL_ONLY = new Set(
  '們這為無與長門問關來對說時會應進還動樣兩經點現將處報張認導設計記讓話語讀課請證誰調論試誤責財貨購費資賬車輪輸轉載較輛運連遲選遞邏邊達醫雙難電龍鳥馬魚風飛飯飲餓館書畫網線終組細紹結給統絕繼績綠緒編縮級紀紅約納紙練羅罰罷聯職聽聲腦臉腳營獲藍慮衛視覺覽觀規訂訓議訊許訪評識訴診詞譯該詳誘諸諾謀謊諧謂謎謝謠謙謹歷壓厭參變號嘆嗎響喚喊噴嗓噓囑囂團圍圖壞塊堅墳墜壟墊牆壯壽塵嘗盡層屬歲崗島嶺峽幣師帳幟帶幫廣莊慶庫廟龐廢開異彌彎彈歸當錄徹徑憶憂懷態總戀懇惡惱驚懼慘懲慚慣憤願懶戲戰撲執擴掃揚擾撫拋搶護擔擬攏揀擁攔撥擇掛摯撓擋掙擠揮撈損撿換搗據擲擱摟攪攜攝擺搖攤撐敵斂數斬斷舊顯曬曉暈暫機殺雜權桿條楊極構樞棗槍楓櫃標棧棟欄樹檔橋樺槳樁夢檢樓檻櫻櫥歡歐殲殘毆畢斃氣匯漢湯溝瀝淪滄滬瀉潑澤潔灑淺漿澆濁測濟渾濃塗濤澇漣渦滌潤澗漲澀淵漁滲灣濕潰濺滾滯滿濾濫濱灘潛瀾滅燈靈燦爐煉熾爍爛燭煩燒燴燙熱煥燜愛爺牽犧狀猶獨狹獅獄獵豬貓獻鹽監蓋盜盤睜瞞矚礦碼磚硯礎碩確礙禮禱禍祿禪離稈種積稱穢稅穩窮竊竅窯竄窩窺豎競篤筍筆籠篩籌簽簡籮簫簍籃籬類糞糧緊糾纖純紗綱縱紛紋紡紐絆紹綁絨繞繪絢絡絞絹繡綏綾綺繩維綿繃綢綜綻綴纜緝緞緩締縷緣縛縫纏繽纓繆繚繳羈聳聾聰肅腸膚腎腫脹脅膽膠脈臟膿膩騰艦艙艱豔藝節蒼蘇莖繭荊薦莢蕩榮葷熒蔭藥萊蓮瑩鶯蘿螢蕭薩蔥蔣薔藹蘊虜雖蝦蝕蟻螞蠶蠻蟄蝸蠟蠅蟬蠍釁銜補襯襖襪襲裝褲貝貞負貢賢敗質販貪貧貶貯貫貳賤貼貴貸貿賀貽賊賈賄賂贓賒賦賭贖賞賜賠賴贅賺賽讚贈贍贏趙趕趨躍踐踴蹤軀軋軌軒軟轟軸輕輔輩輝輥輟輻輯轅轄輾轍辭辯辮遼遷邁違跡遜遺郵鄰鄭鄲醞醬釀釋鑑針釘釣鈣鈍鈔鐘鈉鋼鑰欽鉤鈕錢鉗鑽鐵鈴鉛鉚銅鋁銘銀鑄鋪鏈銷鎖鋤鍋鏽鋒鋅銳錯錨錫錦鍵鋸錘錐錠錳鍬鍛鍍鎮鏡鐮閒悶閘鬧閨聞閩閥閣閡閱闡闊隊陽陰陣階際陸陳陝隕險隨隱隸雛雖雞靂霧霽靜靨韌韓韻頁頂頃項順須顧頓頒頌預顱領頗頸頰頻頹穎顆題額顎顏顛飢飩飪餞飾飽飼飴餌饒餉餃餅餒餛餡饋饞饃餾饅馭馱馴馳驅駁驢駛駙駒駐駝駕驛驍罵驕駱駭騁驗駿騎騙騷騖騾驟驥魷魯鮑鮭鮮鰱鯉鯊鯽鯨鰓鱷鰍鰭鱈鱉鰻鱗鳩鳶鳴鷗鴉鴨鴕鴿鴻鵑鵝鵲鵪鵬鶉鶘鶩鷹鷺麥麩齊齒齡齦齲龔龜體臺學見後裡裏幾雲劃麵鬆闆牠祂妳著佔迴週瞭藉',
);

const CJK = /[一-鿿㐀-䶿]/;
const INTERPOLATION = /\{\{(\w+)\}\}/g;

describe('i18n locale parity', () => {
  it('every locale has exactly the en key set — none missing, none extra', () => {
    const enKeys = new Set(Object.keys(EN));
    for (const [lng, loc] of Object.entries(LOCALES)) {
      if (lng === 'en') continue;
      const keys = new Set(Object.keys(loc));
      const missing = [...enKeys].filter((k) => !keys.has(k));
      const extra = [...keys].filter((k) => !enKeys.has(k));
      expect(missing, `${lng} missing keys`).toEqual([]);
      expect(extra, `${lng} extra keys`).toEqual([]);
    }
  });

  it('no locale has empty or whitespace-only values', () => {
    for (const [lng, loc] of Object.entries(LOCALES)) {
      const empty = Object.entries(loc)
        .filter(([, v]) => typeof v === 'string' && v.trim() === '')
        .map(([k]) => k);
      expect(empty, `${lng} empty values`).toEqual([]);
    }
  });

  it('every locale preserves en interpolation variables per key', () => {
    for (const [lng, loc] of Object.entries(LOCALES)) {
      if (lng === 'en') continue;
      const mismatched = Object.entries(EN).filter(([k, v]) => {
        const enVars = [...v.matchAll(INTERPOLATION)].map((m) => m[1]).sort();
        const locVars = [...(loc[k] ?? '').matchAll(INTERPOLATION)].map((m) => m[1]).sort();
        return JSON.stringify(enVars) !== JSON.stringify(locVars);
      });
      expect(mismatched.map(([k]) => k), `${lng} interpolation mismatches`).toEqual([]);
    }
  });

  it('plural keys are paired: _other has a base or _one singular', () => {
    for (const [lng, loc] of Object.entries(LOCALES)) {
      const keys = new Set(Object.keys(loc));
      const bad = [...keys].filter((k) => {
        if (k.endsWith('_one')) return !keys.has(`${k.slice(0, -4)}_other`);
        if (k.endsWith('_other')) {
          const base = k.slice(0, -6);
          return !keys.has(base) && !keys.has(`${base}_one`);
        }
        return false;
      });
      expect(bad, `${lng} unpaired plural keys`).toEqual([]);
    }
  });
});

describe('i18n residue and script purity', () => {
  it('no untranslated en residue outside the allowlist', () => {
    for (const [lng, loc] of Object.entries(LOCALES)) {
      if (lng === 'en') continue;
      const residue = Object.entries(EN)
        .filter(([k, v]) => loc[k] === v && !SAME_AS_EN.has(k))
        .map(([k]) => k);
      expect(residue, `${lng} identical to en`).toEqual([]);
    }
  });

  it('ms contains no CJK characters', () => {
    const hits = Object.entries(LOCALES.ms)
      .filter(([k, v]) => !SCRIPT_EXEMPT(k) && CJK.test(v))
      .map(([k]) => k);
    expect(hits, 'ms CJK').toEqual([]);
  });

  it('zh-TW contains no Simplified-only characters', () => {
    const hits: string[] = [];
    for (const [k, v] of Object.entries(LOCALES['zh-TW'])) {
      if (SCRIPT_EXEMPT(k)) continue;
      for (const ch of v) if (SIMPLIFIED_ONLY.has(ch)) hits.push(`${k}: '${ch}'`);
    }
    expect(hits, 'zh-TW simplified chars').toEqual([]);
  });

  it('zh-CN contains no Traditional-only characters', () => {
    const hits: string[] = [];
    for (const [k, v] of Object.entries(LOCALES['zh-CN'])) {
      if (SCRIPT_EXEMPT(k)) continue;
      for (const ch of v) if (TRADITIONAL_ONLY.has(ch)) hits.push(`${k}: '${ch}'`);
    }
    expect(hits, 'zh-CN traditional chars').toEqual([]);
  });

  it('zh locales carry CJK for real sentences — no untranslated Latin prose', () => {
    for (const lng of ['zh-CN', 'zh-TW']) {
      const latin = Object.entries(LOCALES[lng])
        .filter(([k, v]) => !SAME_AS_EN.has(k) && !CJK.test(v) && /[a-z]{4,}\s+[a-z]{3,}/.test(v))
        .map(([k]) => k);
      expect(latin, `${lng} Latin-only prose`).toEqual([]);
    }
  });
});
