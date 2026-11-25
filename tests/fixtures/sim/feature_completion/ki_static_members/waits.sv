// Event controls, waits, continuous assignments, ports, assertions and
// coverage on members of static class properties (readme: "Waits").
`timescale 1ns / 1ns
typedef struct packed {
  logic [3:0] hi, lo;
} pair_t;
typedef struct {
  int a;
  logic [3:0] f;
} rec_t;
typedef struct {
  string s;
  real r;
} text_t;
class item;
endclass
class st;
  static pair_t pr = '0;
  static rec_t rec = '{0, 4'h0};
  static text_t tx;
  static rec_t ra[2];
  static pair_t pa[2];
  static int fa[3];
  static item h;
endclass
module sub (
    input [31:0] a,
    output [31:0] y
);
  assign y = a + 1;
endmodule
module tb;
  st o = new;
  int hits = 0, others = 0, events = 0;
  wire [7:0] w;
  wire [31:0] p;
  assign w = st::pr.lo + st::rec.f;
  sub u (
      .a(st::rec.a),
      .y(p)
  );
  always @(st::pr.hi) hits++;
  always @(st::rec.a) others++;
  always @(st::ra[1].a) events++;
  covergroup cg;
    hi: coverpoint st::pr.hi {bins low = {[0 : 3]}; bins high = {[4 : 15]};}
    a: coverpoint st::rec.a {bins zero = {0}; bins some = {[1 : 9]};}
  endgroup
  cg c = new;
  initial begin
    fork
      begin
        wait (st::pr.hi == 3);
        $display("%0t hi", $time);
      end
      begin
        @(o.rec.a);
        $display("%0t record", $time);
      end
      begin
        @(posedge st::pr.lo[1]);
        $display("%0t bit", $time);
      end
      begin
        wait (st::tx.s == "go");
        $display("%0t text", $time);
      end
      begin
        wait (st::fa[1] == 3);
        $display("%0t cell", $time);
      end
      begin
        @(st::ra[1].a);
        $display("%0t element", $time);
      end
      begin
        wait (st::h != null);
        $display("%0t handle", $time);
      end
      begin
        wait (o.pa[1].hi == 4);
        $display("%0t member", $time);
      end
      begin
        #1 st::rec.f = 3;
        #1 st::pr.lo = 1;
        c.sample();
        #1 st::pr.hi = 3;
        #1 o.rec.a = 5;
        #1 st::pr.lo = 2;
        #1 st::tx.s = "go";
        #1 st::fa[0] = 3;
        #1 o.fa[1] = 3;
        #1 st::ra[0].a = 5;
        #1 st::ra[1].a = 6;
        #1 st::h = new;
        #1 o.pa[1].hi = 4;
        c.sample();
      end
    join
    #1 $display("values %0d %0d", w, p);
    $display("counts %0d %0d %0d", hits, others, events);
    $display("coverage %0.2f %0.2f", c.hi.get_coverage(), c.a.get_coverage());
    assert (st::pr.hi == 3 && o.rec.a == 5) $display("assertion");
    $finish;
  end
endmodule
