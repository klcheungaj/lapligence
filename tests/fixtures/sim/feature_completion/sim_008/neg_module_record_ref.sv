// SIM-008: a module or process-block record keeps one storage cell per
// leaf, so it cannot yet bind a native record `ref` formal.
module tb; typedef struct { int a; string s; } r_t; r_t g;
  task automatic upd(ref r_t r); r.a++; r.s = "x"; endtask
  initial begin g.a = 1; upd(g); $display("%0d %s", g.a, g.s); end endmodule
