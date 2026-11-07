class base; int a; function new(int x=0); a=x; endfunction endclass
class der extends base; function new(int x=0); super.new(x); endfunction endclass
module tb;
  function automatic base mk(string s); der d; $display("mk(%s)", s); d=new(7); return d; endfunction
  der qq[$][$];
  der dq[][$];
  initial begin
    der t = new(1);
    qq.push_back({t});
    dq = new[1];
    if ($cast(qq[1][0], mk("a"))) $display("outer end ok %0d", qq.size());
    $display("start");
    if ($cast(dq[0][1], mk("b"))) $display("inner far ok"); else $display("inner far no");
    $display("after");
  end
  final $display("final");
endmodule
