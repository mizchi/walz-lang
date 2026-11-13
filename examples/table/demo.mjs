import { instantiate, op, desc } from "../../runtime/table/index.mjs";

const aq = await instantiate(new URL("../../target/table/", import.meta.url));
const sales = aq.table({
  country: ["JP", "US", "JP", "FR"],
  amount: [20, 100, 22, 3],
  units: [2, 1, 3, 4],
});
const totals = sales
  .filter(d => d.amount >= 10)
  .derive({ revenue: d => d.amount * d.units })
  .groupby("country")
  .rollup({ count: op.count(), revenue: op.sum("revenue") })
  .orderby(desc("revenue"));
console.log(totals.objects());
