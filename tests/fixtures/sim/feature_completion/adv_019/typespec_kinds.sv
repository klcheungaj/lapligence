// ADV-019 typespec kinds (FIN-001 V3 family (d)): vpiPacked of packed and
// unpacked tagged unions (37.23, 7.3.2). typespec_kinds.c prints the
// typespec tree of each variable named below.
module tb;
  typedef union tagged packed { bit [3:0] a; bit [3:0] b; } tp_t;
  typedef union tagged { int i; real r; } tu_t;
  typedef union packed { bit [3:0] a; bit [3:0] b; } up_t;

  tp_t tp;
  tu_t tu;
  up_t up;

  initial begin
    tp = tagged b 4'h5;
    tu = tagged r 1.5;
    up = 4'h3;
    $typespec_kinds;
    $finish;
  end
endmodule
