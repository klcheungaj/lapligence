// Implicit sensitivity through virtual-interface handles (gate-fixes-2 item
// 1): always_comb, always_latch and always @* wake when the member of the
// instance the handle binds changes, or when the handle is rebound, and not
// when an instance the handle does not bind changes (IEEE 1800-2009
// 9.2.2.2.1, 9.4.2.2, 25.9; decision S99-D7).
`timescale 1ns/1ns
interface bus_if;
  logic [7:0] a;
  logic [7:0] b;
  logic [7:0] mem [0:3];
endinterface

module tb;
  bus_if x();
  bus_if y();
  virtual bus_if v = x;
  int i = 1;
  int star_runs = 0;
  logic en = 1'b1;
  logic [7:0] c_a, c_fn, c_el, c_var, c_nib, l_a, s_a;

  function automatic logic [7:0] read_a();
    return v.a;
  endfunction

  always_comb c_a = v.a;
  always_comb c_fn = read_a();
  always_comb c_el = v.mem[2];
  always_comb c_var = v.mem[i];
  always_comb c_nib = {4'h0, v.a[3:0]};
  always_latch if (en) l_a = v.a;
  always @* begin
    s_a = v.a;
    star_runs++;
  end
  // Writes `b` of the bound instance from its `a`.
  always_comb v.b = v.a + 8'd1;

  task automatic show(string what);
    $display("[%0t] %s: a=%h fn=%h el=%h var=%h nib=%h latch=%h star=%h runs=%0d xb=%h yb=%h",
             $time, what, c_a, c_fn, c_el, c_var, c_nib, l_a, s_a, star_runs, x.b, y.b);
  endtask

  initial begin
    #1;
    x.a = 8'h15;
    y.a = 8'h26;
    x.mem[1] = 8'h11;
    x.mem[2] = 8'h12;
    y.mem[1] = 8'h21;
    y.mem[2] = 8'h22;
    #1 show("bound to x");
    x.a = 8'h37;
    x.mem[2] = 8'h32;
    #1 show("x changes");
    i = 2;
    #1 show("index changes");
    y.a = 8'h48;
    y.mem[2] = 8'h42;
    #1 show("unbound y changes");
    v = y;
    #1 show("rebound to y");
    x.a = 8'h59;
    x.mem[2] = 8'h52;
    #1 show("unbound x changes");
    en = 1'b0;
    y.a = 8'h6a;
    y.mem[1] = 8'h61;
    y.mem[2] = 8'h62;
    #1 show("y changes, latch closed");
    i = 1;
    #1 show("index changes back");
    $finish;
  end
endmodule
