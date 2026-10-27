// ADV-019 family 4: VPI statement objects (IEEE 1800-2009 37.56-37.74,
// 37.12), walked by statements.c.
`timescale 1ns/1ns
module tb;
  logic clk = 1'b0;
  logic [7:0] a = 8'd0, b = 8'd3, q;
  logic [3:0] sel = 4'd1;
  int count = 0;
  int arr [3];
  event ev;
  wire [7:0] f;
  assign f = a;
  task automatic bump(input int by);
    count += by;
    -> ev;
  endtask
  function automatic int clamp(input int v);
    if (v > 10) return 10;
    return v;
  endfunction
  function void nothing();
  endfunction
  always #5 clk = ~clk;
  always @(posedge clk or negedge clk) q <= #1 a + b;
  initial begin : main
    int k;
    a = 8'd1;
    for (int i = 0; i < 2; i++) a += 2;
    while (a < 8) a = a << 1;
    repeat (2) @(posedge clk);
    if (a == 8) b = 1; else if (a > 8) b = 2; else b = 3;
    unique case (sel)
      4'd0, 4'd1: count = clamp(count + 20);
      4'd2: ;
      default: begin
        count = 0;
      end
    endcase
    priority if (sel[0]) bump(1);
    fork
      #1 b = 8'd7;
      wait (count > 0) b = 8'd9;
    join
    a = repeat (2) @(posedge clk) b;
    do k++; while (k < 3);
    foreach (arr[j]) arr[j] = j;
    forever begin
      if (k > 5) break;
      k = k + 1;
      continue;
    end
    force q = 8'hff;
    release q;
    @(ev or posedge clk);
    -> ev;
    #2;
    disable main;
  end
  initial begin
    #1 $statements;
    #200 $display("count %0d a %0d b %0d q %0d", count, a, b, q);
    $finish;
  end
endmodule
