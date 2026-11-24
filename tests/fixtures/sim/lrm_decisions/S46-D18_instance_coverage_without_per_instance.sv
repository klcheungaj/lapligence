// S46-D18: get_inst_coverage() of an instance without option.per_instance.
// IEEE 1800-2009 19.8 (SystemVerilog-1800-2009.txt L33571-33572): "the
// get_inst_coverage() method returns the coverage of the specific instance on
// which it is invoked". The example there (L33600-33608) calls it on instances
// of a covergroup that sets no option: "cv1.x.get_inst_coverage(covered,total);
// // total = 2" and "cv1.get_inst_coverage(covered,total); // total = 5".
// Table 19-1 (L33285-33289) gives option.per_instance a meaning for the
// coverage database and report only.
// Decision: the method returns the coverage of the instance, whatever the
// value of option.per_instance.
`timescale 1ns/1ps
module tb;
  int a, b, c, d;
  covergroup cg (int xb, yb, ref int x, y);
    coverpoint x { bins xbins[] = { [0:xb] }; }
    coverpoint y { bins ybins[] = { [0:yb] }; }
  endgroup
  cg cv1, cv2;
  int covered, total;
  real r;
  initial begin
    cv1 = new(1, 2, a, b);
    cv2 = new(3, 6, c, d);
    a = 0; b = 1; cv1.sample();
    a = 1; cv1.sample();
    r = cv1.x.get_inst_coverage(covered, total);
    $display("x %0.2f %0d %0d", r, covered, total);
    r = cv1.y.get_inst_coverage(covered, total);
    $display("y %0.2f %0d %0d", r, covered, total);
    r = cv1.get_inst_coverage(covered, total);
    $display("cv1 %0.2f %0d %0d", r, covered, total);
    r = cv2.get_inst_coverage(covered, total);
    $display("cv2 %0.2f %0d %0d", r, covered, total);
    $finish;
  end
endmodule
