// Decision S22-D8: in an instance, the variables of its generate blocks run
// after its child instances and before the module's own variables, block by
// block in reverse pre-order, as industry-standard simulators run them: a
// block runs the blocks nested in it before its own declarations, and the
// later of two sibling blocks (also loop iterations, an else branch and
// unnamed blocks) runs first. The kind of generate construct does not
// matter. Each block and the module body run their own declarations in
// declaration order. Every initializer calls tick(), which numbers the
// calls in execution order.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". 27.3 L45986-45988: "The elaboration of a generate
// construct results in zero or more instances of a generate block. An
// instance of a generate block is similar in some ways to an instance of a
// module. It creates a new level of hierarchy." The text leaves the order
// among static initializers open.
package util;
  int n;
  function automatic int tick(string s);
    n++;
    $display("%0d: %s", n, s);
    return n;
  endfunction
endpackage
module tb;
  import util::*;
  int v0 = tick("tb.v0");
  for (genvar i = 0; i < 2; i++) begin : f
    int x = tick($sformatf("tb.f[%0d].x", i));
    if (1) begin : in
      int y = tick($sformatf("tb.f[%0d].in.y", i));
      for (genvar j = 0; j < 2; j++) begin : g
        int z = tick($sformatf("tb.f[%0d].in.g[%0d].z", i, j));
      end
    end
    int w = tick($sformatf("tb.f[%0d].w", i));
  end
  int v1 = tick("tb.v1");
  generate
    case (2)
      1: begin : c1
        int x = tick("tb.c1.x");
      end
      2: begin : c2
        int x = tick("tb.c2.x");
        if (1) begin : a
          int x = tick("tb.c2.a.x");
          if (1) begin : b
            int x = tick("tb.c2.a.b.x");
          end
        end
        int y = tick("tb.c2.y");
      end
    endcase
  endgenerate
  if (0) begin : t
    int x = tick("tb.t.x");
  end else begin : e
    int x = tick("tb.e.x");
  end
  if (1) begin
    int x = tick("tb.genblk.x");
  end
  if (1) begin : last
    int x = tick("tb.last.x");
  end
  int v2 = tick("tb.v2");
  initial begin
    #1 $display("done %0d", util::n);
    $finish;
  end
endmodule
