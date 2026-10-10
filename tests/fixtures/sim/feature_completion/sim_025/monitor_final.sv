// SIM-025 A01: a signal that changes several times in the Active and NBA
// regions of one slot yields one $monitor report with the final settled
// values (SV 4.4.2.9, 21.2.3). String and nested-expression arguments are
// re-evaluated for the report.
module tb;
  reg [7:0] a = 8'h01, b = 8'h00;
  reg [3:0] x = 4'b0000;
  real r = 1.0;
  string s = "a";

  function [7:0] twice(input [7:0] v);
    twice = v * 2;
  endfunction

  always @(a) b = a + 8'h01;

  initial begin
    $monitor("M a=%h b=%h x=%b r=%0.2f s=%s len=%0d tw=%0d sum=%0d",
             a, b, x, r, {s, "!"}, s.len(), twice(a), a + b);
    #1;
    a = 8'h10; a = 8'h20;
    x = 4'b1x0z; r = 2.5; s = "bc";
    #1;
    x = 4'bzzzz;
    #1;
    s = "ddd";
    #1;
    r = 0.126;
    #1;
    $display("done");
    $finish(0);
  end

  initial begin
    #1 a <= 8'h30;
  end
endmodule
