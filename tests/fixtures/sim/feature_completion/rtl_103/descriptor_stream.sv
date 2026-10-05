// IEEE 1800-2009 11.4.14, 11.4.14.4: oversized (descriptor-stored) streams
// take runtime `with` selections, dense arrays and rows, unpacked records,
// automatic arrays and nested streams as operands without flattening them.
// Out-of-range selected elements stream the element default; an unknown
// selector selects nothing; a shorter stream is left-justified and
// zero-filled; a two-state destination clears X.
module tb;
  localparam int N = 70000;
  logic [15:0] src [N];
  logic [15:0] dsc [N-1:0];
  logic [15:0] rot [N];
  bit   [15:0] two [N];
  logic [7:0]  bytes [2*N];
  logic [15:0] d2 [3][4];
  logic [15:0] dn [4];
  typedef struct { logic [7:0] p; bit [7:0] q; } pr_t;
  pr_t pr;
  int i, j;
  logic [3:0] ux;
  function automatic logic [31:0] ends(input logic [15:0] v [N]);
    return {v[0], v[N-1]};
  endfunction
  task automatic loc_case;
    logic [15:0] l [4];
    foreach (l[k]) l[k] = 16'hC000 + 16'(k);
    j = 1;
    rot = {>>{l with [j +: 2], src with [0 : N-3]}};
    $display("L %h %h %h %h", rot[0], rot[1], rot[2], rot[N-1]);
  endtask
  initial begin
    foreach (src[k]) src[k] = 16'(k);
    foreach (dsc[k]) dsc[k] = 16'(k);
    foreach (d2[a, b]) d2[a][b] = 16'hD000 + 16'(a * 16 + b);
    foreach (dn[k]) dn[k] = 16'hE000 + 16'(k);
    pr.p = 8'hAB; pr.q = 8'hCD;
    // partly out of bounds on both sides, fallback X
    i = -2;
    rot = {>>{src with [i +: 4], src with [4 : N-1]}};
    $display("A %h %h %h %h %h", rot[0], rot[1], rot[2], rot[3], rot[4]);
    // descending declaration, runtime index, indexed minus
    i = 10;
    rot = {>>{dsc with [i -: 3], dsc with [i], dsc with [N-1 : 4]}};
    $display("B %h %h %h %h %h", rot[0], rot[1], rot[2], rot[3], rot[4]);
    // unknown selector contributes nothing; shorter stream zero-fills
    ux = 4'bx01x;
    rot = {>>{src with [ux +: 2], src with [0 : N-2]}};
    $display("C %h %h %h", rot[0], rot[N-2], rot[N-1]);
    // two-state destination clears X from out-of-bounds source
    i = N - 1;
    two = {>>{src with [i +: 2], src with [0 : N-3]}};
    $display("D %h %h %h", two[0], two[1], two[2]);
    // dense row, packed struct-ish record, dense slice, runtime dense with
    j = 2;
    rot = {>>{d2[1], pr, dn with [j +: 2], src with [0 : N-8]}};
    $display("E %h %h %h %h %h %h %h", rot[0], rot[3], rot[4], rot[5], rot[6], rot[7], rot[N-1]);
    // outer << with runtime part and nested >> / << streams
    i = 3;
    rot = {<<16{src with [i : N-1], {>>{dn[0:1]}}, {<<8{dn with [0 +: 1]}}}};
    $display("F %h %h %h %h", rot[0], rot[1], rot[2], rot[N-1]);
    // narrower cells: two bytes per destination cell
    i = 1;
    bytes = {>>{src with [i : N-1], src with [0 : 0]}};
    $display("G %h %h %h %h", bytes[0], bytes[1], bytes[2*N-2], bytes[2*N-1]);
    // nonblocking with runtime part
    i = 5;
    rot <= {>>{src with [i : N-1], src with [0 : i-1]}};
    i = 0;
    #1 $display("H %h %h", rot[0], rot[N-1]);
    i = 7;
    $display("I %h", ends({>>{src with [i : N-1], src with [0 : i-1]}}));
    loc_case();
    $finish(0);
  end
endmodule
