`timescale 1ns/1ps
module tb;
  reg clk = 0; reg [7:0] m = 0; event e; int cnt;
  class C; rand int x; int lim;
    function int go(); begin : bb localparam L = 6; parameter L2 = L + 1; return L * 10 + L2; end endfunction
    task run(output int o); begin localparam D = 2; #D o = D; end endtask
    static function int sg(); if (1) begin localparam LL = 8; return LL; end endfunction
  endclass
  C c; int r;
  always #1 clk = ~clk;
  initial begin : b
    localparam EDGE = 3; localparam D = 4; localparam B = 2;
    c = new;
    #D $display("t=%0t", $time);
    repeat (B) @(posedge clk);
    $display("t=%0t", $time);
    wait (cnt == B);
    $display("cnt %0d", cnt);
    @(m[B]) $display("bit changed %h", m);
    r = c.randomize() with { x inside {[EDGE:EDGE]}; };
    $display("%0d %0d", r, c.x);
    r = std::randomize(m) with { m == D; };
    $display("%0d %0d", r, m);
    $display("%0d %0d", c.go(), C::sg());
    c.run(r); $display("%0d", r);
    case (m) D: $display("case D"); default: $display("no"); endcase
    m[B +: EDGE] = '1; $display("%b", m);
    m = {EDGE{2'b10}}; $display("%b", m);
    $display("%0d", b.B);
    $finish;
  end
  initial begin #20 cnt = 2; #5 m = 4; end
  function int fn(int q); begin : fb localparam FF = 3; return q * FF; end endfunction
  task tk; begin : tbk localparam FF = 3; parameter GG = 2; $display("task %0d %0d", FF, GG); end endtask
  initial begin $display("fn %0d", fn(4)); tk; end
endmodule
