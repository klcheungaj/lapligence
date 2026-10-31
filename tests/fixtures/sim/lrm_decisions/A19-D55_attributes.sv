// Decision A19-D55: attribute instances (IEEE 1800-2009 5.12, 37.78).
// Companion of A19-D55_attributes.c.
`timescale 1ns/1ns
(* keep, weight = 3, tag = "def" *)
module sub (
  (* pin = 1 *) input wire i,
  output wire o
);
  (* mark = "net" *) wire w;
  (* r_attr = 2'b10 *) logic [1:0] v;
  (* ev_attr *) event e;
  (* fn_attr = 7 *) function int f(input int a); return a; endfunction
  (* prim_attr *) buf b1 (o, i);
  (* proc_attr *) always @(i) v = 2'b01;
  (* ca_attr *) assign w = i;
endmodule
module tb;
  wire x = 1'b1, y;
  (* inst_attr = 5 *) sub s (.i(x), .o(y));
  sub t (.i(x), .o());
  (* real_attr = 1.5, keep = 0 *) reg r;
  initial #1 $d55;
endmodule
