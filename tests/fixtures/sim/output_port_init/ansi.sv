// ANSI output variable ports with declaration initializers, including one
// that depends on a parameter override, on modules, interfaces and programs.
module child #(parameter int P = 7)
  (output logic o = 1'b1, output int n = 5, output logic [7:0] q = P + 1,
   output bit [3:0] b = 4'ha);
endmodule

module explicit_port(.p(a));
  output logic [1:0] a = 2'b10;
endmodule

interface ifc(output logic o = 1'b1, output int n = 3);
endinterface

// A program ends the run when its initial procedures complete (24.3).
program prg(output logic po = 1'b0, output int pn = 4);
  initial #3 $display("program po=%b pn=%0d", po, pn);
endprogram

module tb;
  wire po;
  int pn;
  wire [7:0] pq;
  wire [3:0] pb;
  wire [1:0] pe;
  wire io;
  int in_;
  wire qo;
  int qn;
  bit changed;
  child #(.P(20)) u2(.o(po), .n(pn), .q(pq), .b(pb));
  child u3(.o(), .n(), .q(), .b());
  explicit_port ep(.p(pe));
  ifc i(.o(io), .n(in_));
  prg p(.po(qo), .pn(qn));
  always @(po) changed = 1'b1;
  initial
    $display("tb t0 u2.o=%b u2.n=%0d u2.q=%0d u3.q=%0d", u2.o, u2.n, u2.q, u3.q);
  initial #2 begin
    $display("tb t1 po=%b pn=%0d pq=%0d pb=%h pe=%b changed=%0d", po, pn, pq, pb, pe, changed);
    $display("tb t1 io=%b in=%0d qo=%b qn=%0d", io, in_, qo, qn);
  end
  initial #4 $finish;
endmodule
