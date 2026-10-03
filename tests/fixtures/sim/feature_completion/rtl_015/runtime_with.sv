// IEEE 1800-2009 11.4.14.4: `with` ranges on fixed arrays. Each selector is
// evaluated once, immediately before its array is streamed; a range follows
// the array's declaration (storage) order; a packed range beyond the bounds
// streams element defaults (7.4.6, Table 7-1). Expected values are
// independent bit-string derivations.
module tb;
  typedef struct { bit [3:0] a; logic [3:0] b; } mix_t;
  localparam int N = 70000;

  logic [7:0] asc [0:5];
  logic [7:0] desc [5:0];
  bit [7:0] two [1:3];
  mix_t mix [2];
  logic [15:0] big [N];
  logic [7:0] q [0:3];
  logic [7:0] h8;
  logic [3:0] len;
  logic [63:0] v64;
  logic [31:0] v32;
  int i, j, calls;

  function automatic int next_index(int value);
    calls++;
    return value;
  endfunction

  initial begin
    asc = '{8'h10, 8'h11, 8'h12, 8'h13, 8'h14, 8'h15};
    desc = '{8'h25, 8'h24, 8'h23, 8'h22, 8'h21, 8'h20};
    two = '{8'hA1, 8'hA2, 8'hA3};
    mix[0].a = 4'h1;
    mix[0].b = 4'h2;
    mix[1].a = 4'h3;
    mix[1].b = 4'h4;

    // Sources: every selector form, both directions and both orientations.
    i = 2;
    j = 3;
    v64 = {>>{asc with [i +: 3]}};
    $display("asc_plus %h", v64);
    v64 = {>>{asc with [i -: 2]}};
    $display("asc_minus %h", v64);
    v64 = {>>{asc with [j : 5]}};
    $display("asc_range %h", v64);
    v64 = {>>{asc with [j]}};
    $display("asc_index %h", v64);
    v64 = {>>{desc with [i +: 3]}};
    $display("desc_plus %h", v64);
    v64 = {>>{desc with [j -: 3]}};
    $display("desc_minus %h", v64);
    v64 = {<<4{desc with [i +: 2], asc with [j +: 2]}};
    $display("rl4 %h", v64);
    v64 = {<<3{asc with [i +: 2]}};
    $display("rl3 %h", v64);

    // A source range beyond the bounds streams default elements.
    i = 4;
    v64 = {>>{asc with [i +: 4]}};
    $display("asc_past %b", v64[63:32]);
    i = 2;
    v64 = {>>{two with [i +: 3]}};
    $display("two_past %h", v64);
    i = 0;
    v64 = {>>{mix with [i +: 3]}};
    $display("mix_past %b", v64[63:40]);

    // The selector is evaluated once.
    calls = 0;
    v64 = {>>{asc with [next_index(1) +: 2]}};
    $display("once %h %0d", v64, calls);

    // Targets: runtime ranges, orientation, a later selector that reads a
    // value unpacked to its left, and mixed widths.
    q = '{default: 8'h00};
    i = 1;
    {>>{h8, q with [i +: 2]}} = 24'hA1B2C3;
    $display("tgt_lr %h %h %h %h %h", h8, q[0], q[1], q[2], q[3]);
    {>>{desc with [i +: 3]}} = 24'hD1D2D3;
    $display("tgt_desc %h %h %h %h %h %h", desc[5], desc[4], desc[3], desc[2], desc[1], desc[0]);
    q = '{default: 8'h00};
    {>>{len, q with [0 +: len]}} = 28'h3ABCDEF;
    $display("tgt_dep %h %h %h %h %h", len, q[0], q[1], q[2], q[3]);
    q = '{default: 8'h00};
    i = 2;
    {<<4{h8, q with [i +: 2]}} = 24'h123456;
    $display("tgt_rl4 %h %h %h %h %h", h8, q[0], q[1], q[2], q[3]);
    {<<8{h8, q with [i -: 2]}} = 32'hCAFEF00D;
    $display("tgt_left %h %h %h %h %h", h8, q[0], q[1], q[2], q[3]);

    // Model arrays beyond the dense-cell threshold use descriptor storage.
    big[69990] = 16'hBEEF;
    big[69991] = 16'hCAFE;
    i = 69989;
    v64 = {>>{big with [i +: 4]}};
    $display("big_src %h", v64);
    i = 69998;
    v32 = {>>{big with [i +: 2]}};
    $display("big_past %b", v32[15:0]);
    i = 12345;
    {>>{big with [i +: 2]}} = 32'h0123ABCD;
    $display("big_tgt %h %h %h", big[12344], big[12345], big[12346]);
    $finish(0);
  end
endmodule
