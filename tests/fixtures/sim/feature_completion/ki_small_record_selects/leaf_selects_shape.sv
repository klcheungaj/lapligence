// Only element, member and part selections of a record of integral members
// held as one signal per leaf: each access names its leaf, so the model
// joins no leaves and builds no composite reference (checked on model.c).
typedef struct packed {
  logic [3:0] hi, lo;
} pair_t;

typedef struct {
  logic [7:0] t;
  int a[3];
  pair_t p;
} r_t;

module tb;
  r_t r;
  int k, y;

  initial begin
    r.t = 8'h5a;
    r.a[0] = 1;
    r.a[1] = 2;
    r.a[2] = 3;
    r.p = 8'h3c;
    r.t[3:0] = 4'hf;
    r.p.hi = 4'h9;
    k = 2;
    r.a[k] += 5;
    y = r.a[1] + r.a[k] + r.t[7:4] + r.p.lo;
    $display("%0d %h %0d %h", y, r.t, r.a[2], r.p);
    $finish(0);
  end
endmodule
