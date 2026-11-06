// A randomize() used as a value in an operand other operands can skip, or in
// a loop condition, runs exactly when the operand is evaluated. The expected
// output is the reference run; see the readme for the clauses (11.3.5, 11.4.7,
// 11.4.11, 12.7).
class T; rand bit [3:0] v; bit go = 1; int pre = 0; constraint c { v < 8; }
  function void pre_randomize(); pre++; endfunction endclass
class Arr; rand bit [3:0] a[]; rand T t; constraint c { a.size() inside {[1:3]}; foreach (a[i]) a[i] == i + 1; } function new(); t = new; endfunction endclass
module tb;
  T h, n; Arr r; bit ok; int cnt, i;
  function automatic bit f(); cnt++; return 1; endfunction
  initial begin
    h = new; r = new;
    // && first operand false: not evaluated
    ok = (h.pre > 100) && h.randomize();
    $display("skip && ok=%0d pre=%0d", ok, h.pre);
    ok = (h.pre == 0) && h.randomize();
    $display("run && ok=%0d pre=%0d", ok, h.pre);
    ok = (h.pre == 1) || h.randomize();
    $display("skip || ok=%0d pre=%0d", ok, h.pre);
    ok = (h.pre == 0) || h.randomize();
    $display("run || ok=%0d pre=%0d", ok, h.pre);
    // null receiver guarded
    if (n != null && n.randomize()) $display("bad"); else $display("null guarded");
    if (n == null || n.randomize()) $display("null guarded 2");
    // uvm form
    if ((n == null || !h.go) && !h.randomize() with { v == 3; }) $display("fail"); else $display("v=%0d pre=%0d", h.v, h.pre);
    // conditional arms
    i = (h.pre == 3) ? h.randomize() with { v == 5; } : 7;
    $display("? arm i=%0d v=%0d pre=%0d", i, h.v, h.pre);
    i = (h.pre == 0) ? h.randomize() with { v == 6; } : 8;
    $display("? other i=%0d v=%0d pre=%0d", i, h.v, h.pre);
    i = 1'bx ? h.randomize() with { v == 1; } : 1;
    $display("? x i=%0d v=%0d pre=%0d", i, h.v, h.pre);
    // x first operand evaluates second
    ok = 1'bx && h.randomize();
    $display("x && ok=%b pre=%0d", ok, h.pre);
    // implication
    ok = (h.pre == 0) -> h.randomize();
    $display("-> skip ok=%0d pre=%0d", ok, h.pre);
    // in a loop condition
    i = 0;
    while (h.randomize() with { v == 2; } && i < 3) i++;
    $display("while i=%0d pre=%0d", i, h.pre);
    i = 0;
    do begin if (i == 1) begin i = 5; continue; end i++; end while (i < 4 && h.randomize());
    $display("do i=%0d pre=%0d", i, h.pre);
    for (i = 0; i < 3 && r.randomize(); i++) begin
       if (i == 1) continue;
       $display("for %0d size ok=%0d", i, r.a.size() >= 1 && r.a[0] == 1);
    end
    #1;
    while (!h.randomize() with { v == 4; }) #1;
    $display("v=%0d", h.v);
    $finish(0);
  end
endmodule
