// A VPI application sees the objects of a chosen column record as it sees
// those of its leaf form: the structure, its scalar members, and no object
// for a member array or a string member.
`timescale 1ns / 1ns

typedef struct {
  logic [7:0] tag;
  int n;
  real w;
  string s;
  int a[16];
} r_t;

module tb;
  r_t r;
  int k = 3;

  initial begin
    r.tag = 8'h5a;
    r.n = 7;
    r.w = 1.5;
    r.s = "x";
    r.a[k] = 9;
    #1 $chosen_vpi;
    #1 r.tag = 8'ha5;
    #1 $display("%h %0d %0d", r.tag, r.n, r.a[k]);
    $finish(0);
  end
endmodule
