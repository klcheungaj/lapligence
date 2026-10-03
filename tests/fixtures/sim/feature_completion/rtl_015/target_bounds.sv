// IEEE 1800-2009 11.4.14.4: a `with` range outside a fixed target's bounds
// unpacks only the in-range elements and generates an error, for blocking
// and nonblocking, constant and runtime ranges. An unknown selector writes
// nothing. Results go to stderr so they interleave with the reports.
module tb;
  localparam int STDERR = 32'h8000_0002;
  logic [7:0] u [1:4];
  logic [7:0] w [0:1];
  int i;
  integer k;

  initial begin
    u = '{default: 8'h00};
    w = '{default: 8'h00};
    {>>{u with [3 +: 3]}} = 24'hAABBCC;
    $fdisplay(STDERR, "static %h %h %h %h", u[1], u[2], u[3], u[4]);
    i = 0;
    {>>{u with [i +: 2]}} <= 16'h1122;
    #1;
    $fdisplay(STDERR, "queued %h %h %h %h", u[1], u[2], u[3], u[4]);
    k = 'x;
    {>>{w with [k]}} = 8'h77;
    $fdisplay(STDERR, "unknown %h %h", w[0], w[1]);
    $finish(0);
  end
endmodule
