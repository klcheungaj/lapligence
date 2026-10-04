// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2012_cross_with.sv
// IEEE 1800-2009 A.2.11: cross bins select with binsof/intersect only; a
// `with` select is an IEEE 1800-2012 form.
module tb;
  bit [1:0] a, b;
  covergroup cg;
    ca: coverpoint a;
    cb: coverpoint b;
    x: cross ca, cb { bins same = binsof(ca) with (ca == cb); }
  endgroup
  initial $finish;
endmodule
