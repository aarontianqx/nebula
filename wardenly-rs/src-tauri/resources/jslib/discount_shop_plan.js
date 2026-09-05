// 打折商城购买计划：按内容物名称反查当期商品，输出各目标商品的
// ident/价格/限购/门槛。不在本期的商品不出现在结果里（模板自然跳过）。
//
// args: { "<内容物名称>": true, ... }   例: { "战魂血玉": true, "酒馆礼包": true }
// 返回(JSON): { "<内容物名称>": { ident, package, price, viplevel, level, limitNum, count }, ... }
//
// 商品表取自客户端活动详情（activityType 61）；内容物名称经客户端自己的
// StaticDataUtil.getCostInfo 解析，活动换货/换顺序均不影响解析。
function main(args) {
  var A = __require('Account').default.get().role;
  var U = __require('StaticDataUtil').default;
  var items = A.activity.getActivityInfo(61);
  if (!items) return 'ERR activity detail unavailable (market closed?)';
  var out = {};
  for (var i = 0; i < items.length; i++) {
    var it = items[i];
    if (!it || !it.name || it.price === undefined || !it.reward || !it.reward.length) continue;
    var p = String(it.reward[0]).split('x');
    var c = U.getCostInfo(+p[0], +p[1], +p[2] || 1, 0, false);
    var content = c && c.name ? c.name : '';
    if (args[content]) {
      out[content] = {
        ident: i,
        package: it.name,
        price: it.price,
        viplevel: it.viplevel,
        level: it.level,
        limitNum: it.limitNum,
        count: it.count
      };
    }
  }
  return JSON.stringify(out);
}
