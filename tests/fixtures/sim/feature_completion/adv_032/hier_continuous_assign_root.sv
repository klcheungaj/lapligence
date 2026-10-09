// SystemVerilog-only hierarchical continuous assignments: a `$root` path,
// a child variable (IEEE 1800-2009 6.5 admits one continuous driver) and an
// interface instance's net and variable.
interface bus_if;
  wire [3:0] n;
  logic [3:0] v;
endinterface

module leaf;
  wire [3:0] n;
  logic [3:0] r;
endmodule

module tb;
  logic [3:0] src;
  leaf u ();
  bus_if i ();
  assign $root.tb.u.n = src;
  assign u.r = src + 4'h1;
  assign i.n = ~src;
  assign $root.tb.i.v = src + 4'h2;
  initial begin
    src = 4'h2;
    #1 $display("%h %h %h %h", u.n, u.r, i.n, i.v);
    src = 4'h9;
    #1 $display("%h %h %h %h", u.n, u.r, i.n, i.v);
    $finish;
  end
endmodule
