// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/cover_bins_2009.sv
// IEEE 1800-2009 A.2.11: the bins forms the 2009 grammar admits (ranges with
// iff, transitions, default, binsof/intersect cross selects) stay admitted.
module tb;
  bit [3:0] v;
  bit [1:0] a, b;
  bit en;
  covergroup cg;
    coverpoint v {
      bins low[] = {[0:3]} iff (en);
      bins rise = (0 => 1);
      bins rest = default;
    }
    ca: coverpoint a;
    cb: coverpoint b;
    x: cross ca, cb { bins one = binsof(ca) intersect {1}; ignore_bins skip = !binsof(cb) intersect {0}; }
  endgroup
  initial begin
    $display("bins admitted");
    $finish;
  end
endmodule
