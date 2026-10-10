// SIM-034 A02: synchronous drives and cycle delays on a concrete default
// clocking block, against exact time and region oracles (IEEE 1800-2009
// 14.11, 14.13, 14.16, 4.4). The expected output is derived in readme.md.
`timescale 1ns/1ns
module tb;
  bit clk = 0;
  logic [7:0] d = 8'h00;
  logic [7:0] o = 8'h00, p = 8'h00, q = 8'h00, s = 8'h00;
  logic [7:0] a = 8'h0A, b = 8'h00;
  int n = 1;

  default clocking cb @(posedge clk);
    input d;
    output o, q, s, b;
    output #2 p;
    inout a;
  endclocking

  // Posedges at 10, 20, 25, 40 and 50 (irregular periods).
  initial begin
    #10 clk = 1;
    #5 clk = 0;
    #5 clk = 1;
    #2 clk = 0;
    #3 clk = 1;
    #5 clk = 0;
    #10 clk = 1;
    #5 clk = 0;
    #5 clk = 1;
  end

  always @(posedge clk) d <= d + 8'h01;

  always @(o or p or q or s or a or b)
    $display("%0t M o=%h p=%h q=%h s=%h a=%h b=%h", $time, o, p, q, s, a, b);

  // Runs in the Reactive region, after the Observed clocking event and
  // before the Re-NBA region that commits the drives issued at that edge.
  program observer;
    initial begin
      @(tb.cb);
      $display("%0t R o=%h a=%h b=%h", $time, tb.o, tb.a, tb.b);
      #100;
    end
  endprogram

  initial begin
    @(cb);
    $display("%0t P cb.d=%h d=%h o=%h", $time, cb.d, d, o);
    cb.o <= cb.d + 8'h50;
    cb.p <= 8'h11;
    cb.a <= 8'hA1;
    cb.b <= cb.a;
    cb.q <= ##2 8'h22;
    $strobe("%0t S o=%h p=%h a=%h b=%h", $time, o, p, a, b);
    #0 $display("%0t P #0 o=%h a=%h b=%h", $time, o, a, b);
    #3;
    cb.q <= 8'h33;
    cb.q <= ##2 8'h44;
    cb.s[3:0] <= 4'h7;
    cb.s[7:4] <= ##1 4'h8;
    cb.p <= 8'h12;
    ##1;
    $display("%0t P ##1 cb.d=%h", $time, cb.d);
    ##0;
    $display("%0t P ##0 same step", $time);
    cb.q <= ##0 8'h55;
    #2;
    cb.o <= 8'h60;
    ##0;
    $display("%0t P ##0 waited", $time);
    cb.p <= ##1 8'h77;
    cb.o <= ##(n) 8'h99;
    n = 5;
    ##(n - 3);
    $display("%0t P ##2 done", $time);
    #3 $finish;
  end
endmodule
