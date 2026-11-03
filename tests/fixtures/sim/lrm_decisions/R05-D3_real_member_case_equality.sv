// Decision R05-D3: `===` and `!==` on unpacked structures and arrays with
// real members compare every real leaf bit for bit (its IEEE 754 pattern), as
// they compare integral leaves including X and Z: equal reals match, and 0.0
// and -0.0 do not.
//
// IEEE 1800-2009 11.4.5 (SystemVerilog-1800-2009.txt L14775-14776): "These
// four operators compare operands bit for bit." L14797-14799: "For the case
// equality and case inequality operators (=== and !==), the comparison shall
// be done just as it is in the procedural case statement (see 12.5). Bits
// that are x or z shall be included in the comparison and shall match for the
// result to be considered equal." Table 11-1 (L14254-14255) excludes real
// operands of `===` but not aggregates with real members, and the text gives
// no rule for such a member; llg follows industry-standard behaviour.
module tb;
  typedef struct { int i; real r; } rec_t;
  typedef struct { rec_t in; logic [3:0] l; } outer_t;
  rec_t a, b;
  outer_t o1, o2;
  rec_t qa[$], qb[$];
  real z;
  function automatic bit same(rec_t x, rec_t y);
    return x === y;
  endfunction
  initial begin
    a = '{1, 2.5};
    b = '{1, 2.5};
    $display("eq %0d %0d %0d", a === b, a !== b, same(a, b));
    b.r = 3.0;
    $display("ne %0d %0d %0d", a === b, a !== b, same(a, b));
    z = 0.0;
    a.r = z;
    b.r = -z;
    $display("zero %0d %0d", a === b, same(a, b));
    o1.in = '{1, 2.5};
    o2.in = '{1, 2.5};
    o1.l = 4'b10x1;
    o2.l = 4'b10x1;
    $display("nest %0d", o1 === o2);
    o2.l = 4'b10z1;
    $display("nest2 %0d %0d", o1 === o2, o1 !== o2);
    qa.push_back(a);
    qb.push_back(b);
    $display("queue %0d", qa === qb);
    qb[0].r = z;
    $display("queue2 %0d", qa === qb);
    $finish;
  end
endmodule
