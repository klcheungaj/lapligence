// SIM-020 A01: byte strings, queues, dynamic arrays and nested/reversed
// unpacked arrays round-trip through left and right streams, with type,
// non-power-of-two and constant slice sizes.
module tb;
  string s, r, t;
  byte q[$], p[$];
  byte d[];
  bit [7:0] rev [3:0];
  bit [7:0] fwd [0:3];
  bit [7:0] nest [0:1][0:2];
  bit [7:0] back [0:1][0:2];
  logic [4:0] odd [0:2];
  logic [4:0] odd_back [0:2];
  logic [14:0] packed15;
  logic [15:0] v5;
  shortint sq[$];
  int ok;
  initial begin
    s = "Hello";
    q = {>>{s}};
    $display("string-to-queue %0d %h %h", q.size(), q[0], q[4]);
    r = {<<8{q}};
    $display("queue-to-string %s", r);
    {<<byte{p}} = r;
    t = {>>{p}};
    $display("round-trip %s %0d", t, t == s);
    d = {<<8{s}};
    $display("string-to-dynamic %0d %s", d.size(), string'(d));

    rev = {>>{32'h11223344}};
    $display("reversed-bounds %h %h", rev[3], rev[0]);
    fwd = {<<8{rev}};
    $display("reversed-to-ascending %h %h %h %h", fwd[0], fwd[1], fwd[2], fwd[3]);

    foreach (nest[i, j]) nest[i][j] = 8'h10 * i + j;
    q = {<<8{nest}};
    $display("nested-to-queue %0d %h %h %h", q.size(), q[0], q[2], q[5]);
    {<<8{back}} = q;
    ok = 1;
    foreach (back[i, j]) if (back[i][j] != nest[i][j]) ok = 0;
    $display("nested-round-trip %0d", ok);

    odd[0] = 5'b10000; odd[1] = 5'b00011; odd[2] = 5'b11100;
    packed15 = {<<3{odd}};
    $display("slice3 %b", packed15);
    {<<3{odd_back}} = packed15;
    $display("slice3-back %b %b %b", odd_back[0], odd_back[1], odd_back[2]);

    v5 = {<<5{16'habcd}};
    q = {};
    q.push_back(8'hab);
    q.push_back(8'hcd);
    p = {<<5{q}};
    $display("slice5 %h %0d %h %h", v5, p.size(), p[0], p[1]);

    sq = {<<byte{32'h01020304}};
    $display("byte-slice-to-shortint %0d %h %h", sq.size(), sq[0], sq[1]);
    {>>{q}} = {<<shortint{sq}};
    $display("shortint-slice-to-bytes %0d %h %h %h %h", q.size(), q[0], q[1], q[2], q[3]);
    $finish(0);
  end
endmodule
