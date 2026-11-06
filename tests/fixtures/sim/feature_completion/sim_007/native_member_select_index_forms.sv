// SIM-007: every index form reaches the same element of a module record member
// array, in a short chain and in a decision tree. The index value decides,
// with the index's own signedness and whole width; an index that is not a
// known declared value reads the default and writes nothing (7.4.6).
module tb;
  typedef struct { string c[3]; string t[-2:9]; int n[-2:9]; } r_t;
  r_t r;
  integer k;
  byte sb;
  byte unsigned ub;
  bit [7:0] raw;
  logic [64:0] wide;
  logic signed [64:0] swide;
  logic [63:0] top;
  longint l;
  logic [3:0] unk;
  int idx = 0;
  int q;
  always_comb q = r.n[idx];
  initial begin
    for (k = -2; k <= 9; k++) begin
      r.t[k] = $sformatf("t%0d", k);
      r.n[k] = k * 10;
    end
    for (k = 0; k < 3; k++) r.c[k] = $sformatf("c%0d", k);
    sb = -2;
    $display("[%s] %0d", r.t[sb], r.n[sb]);
    ub = 9;
    $display("[%s] %0d", r.t[ub], r.n[ub]);
    raw = 8'hfe;
    $display("[%s] %0d", r.t[raw], r.n[raw]);
    wide = 65'h1_0000_0000_0000_0003;
    $display("[%s] %0d", r.t[wide], r.n[wide]);
    wide = 3;
    $display("[%s] %0d", r.t[wide], r.n[wide]);
    swide = -1;
    $display("[%s] %0d", r.t[swide], r.n[swide]);
    top = '1;
    $display("[%s] %0d", r.t[top], r.n[top]);
    l = -2;
    $display("[%s] %0d", r.t[l], r.n[l]);
    k = -3;
    $display("[%s] %0d", r.t[k], r.n[k]);
    k = 10;
    $display("[%s] %0d", r.t[k], r.n[k]);
    unk = 4'b0x01;
    $display("[%s] %0d", r.t[unk], r.n[unk]);
    unk = 4'bz000;
    $display("[%s] %0d", r.t[unk], r.n[unk]);
    k = 1;
    $display("[%s] %0d", r.t[k + 1], r.n[k + 1]);
    k = 2;
    $display("[%s]", r.c[k]);
    k = 3;
    $display("[%s]", r.c[k]);
    wide = 65'h1_0000_0000_0000_0003;
    r.t[wide] = "no";
    r.n[wide] = 1;
    swide = -1;
    r.t[swide] = "W";
    r.n[swide] = 7;
    unk = 4'bx;
    r.t[unk] = "no";
    r.n[unk] = 1;
    $display("[%s] [%s] %0d %0d", r.t[3], r.t[-1], r.n[3], r.n[-1]);
    #1 $display("%0d", q);
    idx = 5;
    #1 $display("%0d", q);
    r.n[5] = 55;
    #1 $display("%0d", q);
    idx = 100;
    #1 $display("%0d", q);
    $finish(0);
  end
endmodule
