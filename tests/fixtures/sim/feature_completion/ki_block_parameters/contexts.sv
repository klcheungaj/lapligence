`timescale 1ns/1ps
module sub(input logic [7:0] a, output logic [7:0] o);
  always @* begin : cb localparam M = 8'h0f; parameter [7:0] N = M << 1; o = (a & M) | N; end
endmodule
interface ifc; int v; initial begin : ib localparam IK = 4; v = IK; end endinterface
module tb;
  reg clk = 0; reg [7:0] v = 0, o; event ev; int s; wire [7:0] w; reg [7:0] mem [0:3];
  always #5 clk = ~clk;
  sub u(v, w); ifc i();
  initial begin : b
    localparam K = 5; localparam real TT = 2.5ns; localparam string S = "str";
    static int st = K + 1; automatic int at = K + 2;
    $display("%0d %0d", st, at);
    randcase K: $display("rc"); 0: $display("no"); endcase
    case (K) inside [3:6]: $display("inside"); endcase
    if (K inside {3, 5}) $display("set");
    #TT $display("%0.3f", $realtime);
    v[K] = 1; #1 $display("%h", w);
    repeat (K) s++; $display("%0d", s);
    for (int q = K; q > 0; q -= K) $display("q %0d", q);
    while (s < K + 2) s++; $display("%0d", s);
    force v = K; #1 $display("%h", v); release v;
    mem[K - 3] = K; $display("%0d", mem[2]);
    $display({S, S});
    $display("%s", K > 3 ? S : "n");
    s = S == "str"; $display("%0d", s);
    @(posedge clk iff K == 5); $display("iff");
    assert property (@(posedge clk) K == 5);
    fork : fk begin localparam D = 1; #D $display("f1"); end begin localparam D = 2; #D $display("f2"); disable fk; end #K $display("never"); join
    s = fv(K); $display("%0d", s);
    tk(K, s); $display("%0d", s);
    {>>{v}} = 8'h5a; s = {<<{v[K:0]}}; $display("%h", s);
    unique if (K == 5) $display("u"); else if (K == 6) $display("x");
    ->> #K ev; @ev $display("ev %0t", $time);
    $display("%0d", i.ib.IK);
    begin : d parameter K2 = K ** 2; do begin localparam Z = K2; s = Z; break; end while (1); $display("%0d", s); end
    $finish;
  end
  function int fv(int x); begin localparam F = 3; if (x) begin : qq localparam F = 4; return F * x; end return F; end endfunction
  task automatic tk(int x, output int y); fork begin localparam F = 3; #F y = F + x; end join endtask
endmodule
