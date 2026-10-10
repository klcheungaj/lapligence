// SIM-032: package access, class declarations, nested programs sharing
// module variables and hierarchical references between programs
// (IEEE 1800-2009 24.3, 24.5).
package cfg;
  int scale = 3;
  function automatic int apply(int x);
    return x * scale;
  endfunction
endpackage

program a(input int seed);
  import cfg::*;
  int shared = 11;
  class Box;
    int v;
    function new(int x);
      v = x;
    endfunction
    task automatic show(int d);
      #d $display("box v=%0d t=%0d", v, $time);
    endtask
  endclass
  initial begin
    Box bx;
    bx = new(apply(seed));
    bx.show(1);
    #2 $display("a sees b.x=%0d t=%0d", tb.b0.x, $time);
  end
endprogram

program b;
  int x = 22;
  initial #2 $display("b sees a.shared=%0d t=%0d", tb.a0.shared, $time);
endprogram

module tb;
  int seed = 4;
  int common = 7;
  a a0(.seed(seed));
  b b0();
  program nested;
    initial #4 $display("nested sees common=%0d t=%0d", common, $time);
  endprogram
endmodule
