// Decision R05-D2: an ambiguous conditional on queues, dynamic arrays and
// associative arrays merges them element by element. Queues and dynamic
// arrays merge position by position and the result has the larger size;
// associative arrays merge to the union of their keys. An element that
// matches in both arms keeps its value; an element that differs, holds X or
// Z, or exists in only one arm takes its default-uninitialized value.
//
// IEEE 1800-2009 11.4.11 (SystemVerilog-1800-2009.txt L15211, L15214-15216):
// "For nonintegral and aggregate expressions, if cond_predicate evaluates to
// an ambiguous value, then: ... both the first expression and second
// expression shall be evaluated, and their results shall be combined element
// by element. If the elements match, the element is returned. If they do not
// match, then the default-uninitialized value for that element's type shall
// be returned." The text does not say what an element present in only one
// arm combines with; llg merges it with the element's default, so its
// integral, string and record leaves take their defaults (industry-standard
// behaviour for the result size and keys).
//
// Every differing integral element below differs in all of its bits, so the
// default (X for logic, 0 for int) is also what merging its bits would give.
module tb;
  logic c;
  int qa[$], qb[$], qr[$];
  logic [3:0] da[], db[], dr[];
  int aa[int], ab[int], ar[int];
  string sa[$], sb[$], sr[$];
  initial begin
    qa = '{1, 5, 3};
    qb = '{1, 10, 3, 4};
    da = new[2];
    db = new[2];
    da[0] = 4'h5; da[1] = 4'h7;
    db[0] = 4'ha; db[1] = 4'h7;
    aa[3] = 30; aa[-1] = 7;
    ab[3] = 30; ab[5] = 50;
    sa = '{"x", "y"};
    sb = '{"x", "z", "w"};
    c = 1'bx;
    qr = c ? qa : qb;
    dr = c ? da : db;
    ar = c ? aa : ab;
    sr = c ? sa : sb;
    $display("q %0d: %0d %0d %0d %0d", qr.size(), qr[0], qr[1], qr[2], qr[3]);
    $display("d %0d: %b %b", dr.size(), dr[0], dr[1]);
    $display("a %0d: %0d %0d %0d", ar.size(), ar[-1], ar[3], ar[5]);
    $display("s %0d: [%s] [%s] [%s]", sr.size(), sr[0], sr[1], sr[2]);
    c = 1'bz;
    qr = c ? qb : qa;
    $display("z %0d: %0d %0d %0d %0d", qr.size(), qr[0], qr[1], qr[2], qr[3]);
    c = 1'b1;
    qr = c ? qa : qb;
    ar = c ? aa : ab;
    $display("t %0d %0d %0d", qr.size(), qr[1], ar.size());
    c = 1'b0;
    qr = c ? qa : qb;
    ar = c ? aa : ab;
    $display("f %0d %0d %0d", qr.size(), qr[1], ar[5]);
    $finish;
  end
endmodule
