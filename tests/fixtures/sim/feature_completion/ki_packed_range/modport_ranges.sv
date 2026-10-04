// RTL-102 finding: a modport expression port over an element range,
// `.p(w[3:2])`, names elements 3 and 2 of `w` and takes the range's
// self-determined type `logic [3:2][7:0]` (IEEE 1800-2009 7.4.5, 25.5.4).
interface bus_if;
  logic [3:0][7:0] w;
  modport wr(output .p(w[3:2]));
  modport rd(input .p(w[3:2]), input .q(w[1:0]), input .lane(w[2]));
endinterface
module writer(bus_if.wr b);
  integer k;
  initial begin
    #2 b.p = 16'hbeef;
    #2 b.p[2] = 8'h11;
    k = 3;
    #2 b.p[k] = 8'h22;
    #2 b.p[k -: 2] = 16'h3344;
  end
endmodule
module reader(bus_if.rd b, output logic [15:0] seen, output logic [7:0] lane);
  assign seen = b.p;
  assign lane = b.lane;
  // Skip time zero, where the initial write races this block's first wait.
  always @(b.p) if ($time > 0) $display("change p=%h p[2]=%h q=%h", b.p, b.p[2], b.q);
endmodule
module tb;
  bus_if bi();
  logic [15:0] seen;
  logic [7:0] lane;
  writer wr(bi);
  reader rd(bi, seen, lane);
  initial begin
    bi.w = 32'h44332211;
    #1 $display("sample w=%h seen=%h lane=%h", bi.w, seen, lane);
    repeat (4) #2 $display("sample w=%h seen=%h lane=%h", bi.w, seen, lane);
    $finish(0);
  end
endmodule
