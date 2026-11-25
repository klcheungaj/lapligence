// Members, bits and parts of packed static class properties, named through
// the class and through an object (readme: "Packed properties").
`timescale 1ns / 1ns
typedef enum logic [1:0] {RED, GREEN, BLUE} color_t;
typedef struct packed {
  logic [3:0] hi, lo;
} pair_t;
typedef struct packed {
  pair_t p;
  logic [7:0] b;
} nest_t;
typedef union packed {
  logic [7:0] w;
  pair_t p;
} flat_t;
typedef struct packed {
  color_t c;
  logic signed [5:0] x;
} tint_t;
typedef union tagged packed {
  logic [7:0] a;
  logic [7:0] b;
} choice_t;
class st;
  static pair_t pr;
  static nest_t ne;
  static flat_t fl;
  static tint_t ti;
  static choice_t ch;
  static pair_t [2:0] pa;
  static logic [1:0][7:0] md;
  static logic [99:0] wide;
  static pair_t ini = '{hi: 4'h3, lo: ini_lo()};
  static function logic [3:0] ini_lo();
    return 4'h9;
  endfunction
endclass
module tb;
  st o = new;
  int i;
  initial begin
    // Writing one member keeps the others, whatever names the property.
    st::pr = 8'h00;
    st::pr.hi = 3;
    o.pr.lo = 5;
    $display("pair %h %h %h", st::pr, o.pr.hi, st::pr.lo);
    i = 1;
    st::pr.hi[2] = 1;
    o.pr.lo[3:2] = 2;
    st::pr.hi[i-:2] = 2;
    $display("bits %b %b %b %b", o.pr, st::pr.hi[2], o.pr.lo[3:2], st::pr.hi[i]);
    st::pr.hi += 2;
    o.pr.lo++;
    i = st::pr.hi++;
    $display("steps %h %0d %h", st::pr, i, ++o.pr.lo);
    st::ne = '0;
    st::ne.p.hi = 4;
    o.ne.b = 8'h5a;
    o.ne.p.lo[1:0] = 2;
    $display("nested %h %h %h %h", st::ne, o.ne.p, st::ne.p.hi, o.ne.b);
    st::fl.w = 8'h3c;
    o.fl.p.hi = 9;
    $display("union %h %h", st::fl.w, st::fl.p.lo);
    st::ti.c = GREEN;
    o.ti.x = -3;
    $display("enum %h %s %0d %s", st::ti, st::ti.c.name(), st::ti.x, o.ti.c.next().name());
    st::ch = tagged a 8'h12;
    o.ch.a = 8'h34;
    $display("tagged %h", st::ch.a);
    st::pa = '0;
    i = 2;
    st::pa[1].hi = 3;
    o.pa[i].lo = 7;
    st::pa[0] = 8'hee;
    $display("array %h %h %h %h", st::pa, st::pa[1].hi, o.pa[i], o.pa[i].lo);
    i = 1;
    st::md = '0;
    st::md[1][3:0] = 5;
    o.md[0] = 8'h77;
    st::md[i][7] = 1;
    $display("dimensions %h %h %h", st::md, o.md[i], st::md[1][3:0]);
    st::wide = '0;
    i = 70;
    st::wide[70] = 1;
    o.wide[i+10] = 1;
    st::wide[95:90] = 6'h2a;
    $display("wide %h", st::wide);
    // Concatenations, patterns, streams, casts and comparisons.
    {st::pr.hi, o.pr.lo} = 8'h5a;
    $display("concat %h %h", st::pr, {o.pr.lo, st::pr.hi});
    o.pr = '{hi: 1, lo: 2};
    $display("pattern %h", st::pr);
    {>>{st::pr.lo, st::pr.hi}} = 8'h12;
    $display("stream %h %h", o.pr, pair_t'({<<4{st::pr}}));
    $display("query %0d %0d %0d %0d", $bits(st::pr.hi), $left(o.pr.lo), $countones(st::pr.hi),
             $clog2(o.pr.hi));
    $display("inside %b %b", st::pr.hi inside {[2:4]}, 2 inside {o.pr.hi, st::pr.lo});
    case (st::pr.hi)
      2: $display("case two");
      default: $display("case other");
    endcase
    $display("initial %h", st::ini);
  end
endmodule
