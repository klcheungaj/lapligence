// llg-test-fixture: tests/fixtures/sim/syn011_rtl_helper_events/observed_result.sv
// Static functions whose result variable a dump or a VPI application can observe
// are evaluated privately by event controls and reports (IEEE 1800-2009 9.4.2,
// 13.4.2, 21.2.3).
module tb;
  reg [3:0] t = 0; integer n = 0, m = 0, k = 0;
  function [3:0] part(input [3:0] v);
    begin part = 0; part[0] = v[1]; part[2:1] = v[3:2]; part[3 -: 1] = v[0]; end
  endfunction
  function [3:0] cond(input [3:0] v);
    begin if (v > 4) cond = 1; else begin cond = 2; end end
  endfunction
  function integer rr(input [3:0] v);
    for (int i = 0; i < 2; i = i + 1) begin if (v == i) return 7; end
    return 8;
  endfunction
  function real fr(input [3:0] v); fr = v / 2; endfunction
  always @(part(t)) n = n + 1;
  always @(cond(t)) m = m + 1;
  always @(rr(t)) k = k + 1;
  always @(fr(t)) $display("fr");
  initial begin
    #1 t = 1; #1 t = 5; #1 t = 6; #1 t = 0;
    #1 $display("%0d %0d %0d part=%h", n, m, k, part(4'b1010));
    $monitor("mon %0d %0d", cond(t), part(t));
    #1 t = 7; #1 $finish(0);
  end
endmodule
