class base; int a; function new(int x=0); a=x; endfunction endclass
class der extends base; function new(int x=0); super.new(x); endfunction endclass
class o2 extends base; function new(int x=0); super.new(x); endfunction endclass
module tb;
  typedef enum {RED=1, GREEN=2, BLUE=4} color_t;
  function automatic base mk(string s, int k); der d; o2 o; $display("mk(%s)", s); if (k==1) begin d=new(7); return d; end o=new(8); return o; endfunction
  der qq[$][$];
  der dq[][$];
  der aq[int][$];
  color_t cq[$][$];
  int n;
  initial begin
    der t = new(1);
    qq.push_back({t});
    if ($cast(qq[0][1], mk("a",1))) $display("inner end ok %0d %0d", qq[0].size(), qq[0][1].a);
    if ($cast(qq[0][2], mk("b",2))) $display("fail ok"); else $display("inner end fail %0d", qq[0].size());
    if ($cast(qq[1][0], mk("c",1))) $display("outer end ok %0d %0d %0d", qq.size(), qq[1].size(), qq[1][0].a);
    $cast(qq[2][0], t);
    $display("upcast %0d %0d", qq.size(), qq[2][0].a);
    dq = new[2];
    if ($cast(dq[1][0], mk("d",1))) $display("dyn ok %0d %0d", dq[1].size(), dq[1][0].a);
    if ($cast(aq[4][0], mk("e",1))) $display("assoc ok %0d %0d %0d", aq.num(), aq[4].size(), aq[4][0].a);
    if ($cast(aq[5][0], mk("f",2))) $display("x"); else $display("assoc fail %0d %0d", aq.num(), aq.exists(5));
    n = 2;
    cq.push_back({});
    if ($cast(cq[0][0], n)) $display("enum ok %0d %s", cq[0].size(), cq[0][0].name());
    n = 3;
    if ($cast(cq[0][1], n)) $display("x"); else $display("enum fail %0d", cq[0].size());
    $display("done");
    $finish;
  end
endmodule
