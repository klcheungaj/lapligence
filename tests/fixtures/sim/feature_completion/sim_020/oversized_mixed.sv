// SIM-020 A03: resizable operands of oversized descriptor streams and
// oversized fixed unpack targets stream as runtime-sized cells; no packed
// temporary of the whole stream is formed.
module tb;
  localparam int N = 70000;
  logic [15:0] src [N];
  logic [15:0] rot [N];
  logic [15:0] back [N];
  logic [15:0] nb [N];
  logic [15:0] q [$];
  int k;
  initial begin
    foreach (src[i]) src[i] = i[15:0];
    q.push_back(16'hbeef);
    rot = {>>{q, src with [0 : N-2]}};
    $display("queue-operand %h %h %h %h", rot[0], rot[1], rot[2], rot[N-1]);
    rot = {<<16{q, src with [0 : N-2]}};
    $display("queue-operand-reversed %h %h %h", rot[0], rot[N-2], rot[N-1]);
    q = {16'h1111, 16'h2222, 16'h3333};
    k = 1;
    rot = {>>{q with [k +: 2], src with [0 : N-3]}};
    $display("selected-queue-operand %h %h %h %h", rot[0], rot[1], rot[2], rot[N-1]);
    {<<16{back}} = {>>{q, src}};
    $display("oversized-target %h %h %h %h", back[0], back[1], back[N-3], back[N-1]);
    #5 $finish;
  end
  initial begin
    #2 {>>{nb}} <= {<<16{src}};
    #1 $display("oversized-target-nba %h %h", nb[0], nb[N-1]);
  end
endmodule
