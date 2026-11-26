// The implicit sensitivity of always_comb and always @*, and the sampled
// value of a member (readme: "Waits").
`timescale 1ns / 1ns
typedef struct packed {
  logic [3:0] hi, lo;
} pair_t;
typedef struct {
  int a;
  logic [3:0] f;
} rec_t;
class st;
  static pair_t pr = '0;
  static rec_t rec = '{0, 4'h0};
  static rec_t ra[2];
  static int fa[3];
endclass
module tb;
  st o = new;
  int y, z, m, runs = 0;
  logic ck = 0;
  always_comb begin
    y = st::pr.hi + o.rec.a;
    runs++;
  end
  always_comb z = st::fa[1] + st::ra[1].a;
  always @* m = st::pr.lo * 2;
  always @(posedge ck) if ($rose(st::pr.hi == 4)) $display("rose %0t", $time);
  initial begin
    #1 $display("%0d %0d", y, runs);
    st::pr.lo = 3;  // Not read by the first process.
    #1 $display("%0d %0d %0d", y, runs, m);
    o.pr.hi = 3;
    #1 st::rec.a = 4;
    #1 $display("%0d %0d %0d", y, runs, m);
    o.fa[1] = 5;
    #1 st::ra[1].a = 6;
    #1 $display("%0d", z);
    ck = 1;
    st::pr.hi <= 4;
    #1 ck = 0;
    #1 ck = 1;
    #1 $finish;
  end
endmodule
