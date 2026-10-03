// IEEE 1800-2009 6.24.3 and 11.4.14.1-11.4.14.3: packing, unpacking and
// bit-stream casts of fixed data in value, argument, port and lvalue
// contexts. Expected values are independent bit-string derivations.
module reverse5(input logic [15:0] i, output logic [15:0] o);
  assign o = {<<5{i}};
endmodule

module const16(output logic [15:0] o);
  assign o = 16'hABCD;
endmodule

module tb;
  typedef struct packed { logic [3:0] hi; logic [2:0] lo; } p7_t;
  typedef struct { logic [4:0] tag; bit [7:0] row [2]; p7_t p; } rec_t;
  typedef struct { rec_t r; logic [3:0] tail; } outer_t;
  typedef logic [3:0] nib_t [8];
  typedef bit [2:0] tri_t [0:4];
  typedef logic [31:0] w32_t;

  logic [15:0] v16, port_out;
  logic [31:0] v32;
  logic [7:0] d [3:0];
  logic [7:0] u [0:3];
  outer_t o;
  nib_t n;
  tri_t t;
  logic [2:0] a3;
  logic [8:0] b9;
  logic [19:0] c20;
  bit [47:0] wide;
  bit [39:0] two;
  logic [7:0] x8, y8;
  logic [7:0] sw [0:3];
  logic [7:0] pa, pb;
  logic [7:0] pq [0:3];

  function automatic logic [31:0] pass(input logic [31:0] v);
    return v;
  endfunction

  task automatic split(output logic [15:0] hi, output logic [15:0] lo, input logic [31:0] v);
    {hi, lo} = v;
  endtask

  reverse5 r5(.i({<<8{x8, y8}}), .o(port_out));
  const16 c0(.o({<<8{pa, pb}}));
  const16 c1(.o({>>{pq with [1 +: 2]}}));

  initial begin
    // Non-dividing and type slice sizes (11.4.14.2).
    v16 = {<<3{16'hB6E5}};
    $display("slice3 %h", v16);
    v16 = {<<5{16'hB6E5}};
    $display("slice5 %h", v16);
    v16 = {<<7{16'hB6E5}};
    $display("slice7 %h", v16);
    v32 = {<<byte{32'h11223344}};
    $display("byte %h", v32);
    v32 = {<<shortint{32'h11223344}};
    $display("shortint %h", v32);
    v16 = {<<p7_t{16'hB6E5}};
    $display("type7 %h", v16);

    // Descending and ascending fixed arrays stream in declaration order.
    d = '{8'hA1, 8'hB2, 8'hC3, 8'hD4};
    u = '{8'h01, 8'h23, 8'h45, 8'h67};
    v32 = {>>{d}};
    $display("d_lr %h", v32);
    v32 = {<<8{d}};
    $display("d_rl8 %h", v32);
    v32 = {<<3{d}};
    $display("d_rl3 %h", v32);
    v32 = {<<12{u}};
    $display("u_rl12 %h", v32);
    {<<5{d}} = 32'h89ABCDEF;
    $display("d_set %h %h %h %h", d[3], d[2], d[1], d[0]);
    {>>{u}} = {d[0], d[1], d[2], d[3]};
    $display("u_set %h %h %h %h", u[0], u[1], u[2], u[3]);

    // Nested fixed records.
    o.r.tag = 5'h15;
    o.r.row[0] = 8'h3C;
    o.r.row[1] = 8'hA5;
    o.r.p = 7'h5A;
    o.tail = 4'h9;
    v32 = {>>{o}};
    $display("o_lr %h", v32);
    v32 = {<<4{o}};
    $display("o_rl4 %h", v32);
    {<<7{o}} = 32'h13579BDF;
    $display("o_set %h %h %h %h %h", o.r.tag, o.r.row[0], o.r.row[1], o.r.p, o.tail);

    // Bit-stream casts (6.24.3).
    n = nib_t'(32'hFEDCBA98);
    $display("cast_n %h %h %h", n[0], n[3], n[7]);
    v32 = w32_t'(n);
    $display("cast_back %h", v32);
    o = outer_t'(32'h2468ACE1);
    $display("cast_o %h %h %h %h %h", o.r.tag, o.r.row[0], o.r.row[1], o.r.p, o.tail);
    t = tri_t'(15'h5A3C);
    $display("cast_t %0d %0d %0d %0d %0d", t[0], t[1], t[2], t[3], t[4]);

    // Mixed destination widths in both directions.
    {>>{a3, b9, c20}} = 32'hC0FFEE42;
    $display("mixed_lr %h %h %h", a3, b9, c20);
    {<<5{a3, b9, c20}} = 32'hC0FFEE42;
    $display("mixed_rl5 %h %h %h", a3, b9, c20);

    // A wider target is left-aligned and zero-filled; a wider source is
    // consumed from its left end.
    wide = {>>{u}};
    $display("wide_u %h", wide);
    wide = {<<4{a3, b9}};
    $display("wide_rl4 %h", wide);
    {>>{x8}} = 32'hDEADBEEF;
    $display("left_lr %h", x8);
    {<<8{x8, y8}} = 24'hAABBCC;
    $display("left_rl8 %h %h", x8, y8);
    {<<4{x8}} = 16'h1234;
    $display("left_rl4 %h", x8);

    // Source X/Z states survive four-state targets and convert for two-state ones.
    v16 = 16'b10xz_zx01_1x0z_0011;
    two = {>>{v16, v16}};
    $display("two %b", two);
    v32 = {<<4{v16, v16}};
    $display("four %b", v32);

    // Overlapping sources and destinations use the value read before any write.
    x8 = 8'h12;
    y8 = 8'h34;
    {>>{x8, y8}} = {y8, x8};
    $display("swap %h %h", x8, y8);
    sw = '{8'h01, 8'h02, 8'h03, 8'h04};
    {<<8{sw}} = sw;
    $display("rev %h %h %h %h", sw[0], sw[1], sw[2], sw[3]);
    {>>{sw[1], sw[2]}} = {<<8{sw[1], sw[2]}};
    $display("inner %h %h %h %h", sw[0], sw[1], sw[2], sw[3]);

    // Argument and port contexts.
    v32 = pass({<<16{u}});
    $display("arg %h", v32);
    split({>>{x8, y8}}, v16, 32'hCAFEF00D);
    $display("out_arg %h %h %h", x8, y8, v16);
    #1;
    $display("port %h", port_out);
    $display("port_lvalue %h %h", pa, pb);
    $display("port_with %h %h %h %h", pq[0], pq[1], pq[2], pq[3]);
    $finish(0);
  end
endmodule
