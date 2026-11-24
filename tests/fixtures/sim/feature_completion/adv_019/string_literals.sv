// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_019/string_literals.sv
// String literals with NUL, control and non-UTF-8 characters in the VPI
// catalog (IEEE 1800-2009 5.9, 6.16, 37.55).
module tb;
  string s;
  reg [63:0] v;
  parameter string PN = "a\000b\000";
  parameter string PB = "back\\slash\"q\"";
  parameter string PX = "\377z";
  parameter string PE = "";
  class C;
    localparam string CN = "c\000d\\";
    localparam CV = "e\000f";
  endclass
  (* text = "x\000\"y\\" *) reg marked;
  initial begin
    s = "a\000b";
    s = "";
    s = "\000";
    s = "\000007";
    s = "q\"b\\n\nt\tv\vf\fa\ad\177";
    s = "\377\200";
    s = "\303\251";
    v = {"a\000b", 8'h41};
  end
  initial begin
    #1 v = "a\000b";
    $display("%0d <%s> <%s> %h %h", PN.len(), PN, PB, PX.getc(0), v);
    $lit(v);
    #1 $display("%h", v);
    $finish(0);
  end
endmodule
