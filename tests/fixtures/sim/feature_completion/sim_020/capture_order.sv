// SIM-020 A02: overlapping sources and destinations read the whole source
// before any target is written; `with` selectors evaluate once, in stream
// order, immediately before their array is streamed.
module tb;
  byte q[$], p[$];
  byte a [0:3];
  byte b [0:3];
  byte d[];
  byte x, y;
  int i;
  logic [15:0] v;
  function automatic int next_index();
    i = i + 1;
    return i;
  endfunction
  initial begin
    q = {8'h01, 8'h02, 8'h03, 8'h04};
    {>>{q with [0 +: 2], p}} = {>>{q}};
    $display("overlap-with %0d %0d %h %h %h %h", q.size(), p.size(), q[0], q[1], p[0], p[1]);
    q = {8'h01, 8'h02, 8'h03};
    q = {<<8{q}};
    $display("self-reverse %0d %h %h %h", q.size(), q[0], q[1], q[2]);
    {<<8{q}} = {>>{q, 8'h04}};
    $display("self-grow %0d %h %h %h %h", q.size(), q[0], q[1], q[2], q[3]);
    x = 8'h11;
    y = 8'h22;
    {>>{x, y}} = {>>{y, x}};
    $display("swap %h %h", x, y);
    foreach (a[k]) a[k] = 8'h10 + k;
    {>>{a}} = {<<8{a}};
    $display("fixed-self-reverse %h %h %h %h", a[0], a[1], a[2], a[3]);

    foreach (b[k]) b[k] = 8'h10 + k;
    i = 0;
    v = {>>{b with [next_index() +: 2]}};
    $display("source-selector %h %0d", v, i);
    foreach (b[k]) b[k] = 0;
    i = 0;
    {>>{b with [next_index() +: 2]}} = 16'haabb;
    $display("target-selector %h %h %h %h %0d", b[0], b[1], b[2], b[3], i);
    i = 0;
    {>>{d with [0 +: next_index()]}} = 16'haabb;
    $display("dynamic-selector %0d %h %0d", d.size(), d[0], i);
    i = 0;
    p = {};
    {<<8{p with [0 +: next_index()], x}} = 16'h1234;
    $display("reverse-selector %0d %h %h %0d", p.size(), p[0], x, i);
    $finish;
  end
endmodule
