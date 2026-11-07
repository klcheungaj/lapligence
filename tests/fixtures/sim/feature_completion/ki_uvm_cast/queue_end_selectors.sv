class base; int a; function new(int x=0); a=x; endfunction endclass
class der extends base; function new(int x=0); super.new(x); endfunction endclass
module tb;
  typedef enum {RED=1, GREEN=2, BLUE=4} color_t;
  der q[$];
  der qq[$][$];
  color_t cq[$];
  function automatic base grow(int v); der d = new(v); der e = new(5); q.push_back(e); return d; endfunction
  function automatic base growq(int v); der d = new(v); der e = new(6); qq.push_back({e}); return d; endfunction
  function automatic int growc(int v); cq.push_back(BLUE); return v; endfunction
  function automatic void show(string tag); string s; foreach (q[i]) s = {s, $sformatf(" %0d", q[i].a)}; $display("%s %0d:%s", tag, q.size(), s); endfunction
  initial begin
    der t = new(1);
    q = {t, t};
    if ($cast(q[$], grow(9))) show("end");
    if ($cast(q[$-1], grow(8))) show("end-1");
    if ($cast(q[$+1], grow(7))) show("end+1");
    qq.push_back({t});
    if ($cast(qq[$][0], growq(4))) $display("%0d: %0d %0d", qq.size(), qq[0][0].a, qq[1][0].a);
    cq = {RED};
    if ($cast(cq[$], growc(2))) $display("%0d: %s %s", cq.size(), cq[0].name(), cq[1].name());
    $finish;
  end
endmodule
