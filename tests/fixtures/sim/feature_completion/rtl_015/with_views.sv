// IEEE 1800-2009 11.4.14.4, 13.5.2 and 7.4.6: `with` ranges on one-dimensional
// fixed arrays reached through ref and const-ref formals, automatic locals,
// record members, rows of a two-dimensional array and function results, as
// sources and as unpack targets. Expected values are independent bit-string
// derivations.
module tb;
  typedef logic [7:0] row_t [0:3];
  typedef struct { logic [3:0] h; logic [7:0] arr [2:0]; } rec_t;

  logic [7:0] m [0:3];
  logic [7:0] grid [2][0:3];
  rec_t r;
  logic [31:0] v32;
  logic [15:0] v16;
  int i;

  function automatic logic [31:0] ref_source(int at, const ref logic [7:0] s [0:3]);
    return {>>{s with [at +: 2]}};
  endfunction

  function automatic void ref_target(int at, ref logic [7:0] t [0:3]);
    {<<8{t with [at +: 2]}} = 16'hABCD;
  endfunction

  function automatic void ref_static(ref logic [7:0] t [0:3]);
    {>>{t with [3 : 3]}} = 8'h99;
  endfunction

  function automatic logic [31:0] local_view(int at);
    logic [7:0] loc [0:3];
    loc = '{8'h51, 8'h52, 8'h53, 8'h54};
    {>>{loc with [at +: 2]}} = 16'hE1E2;
    return {>>{loc with [at - 1 +: 3]}};
  endfunction

  function automatic row_t make_row(logic [7:0] base);
    return '{base, base + 8'd1, base + 8'd2, base + 8'd3};
  endfunction

  initial begin
    m = '{8'h11, 8'h22, 8'h33, 8'h44};
    i = 1;
    v32 = ref_source(i, m);
    $display("const_ref %h", v32);
    v32 = ref_source(3, m);
    $display("const_ref_past %b", v32);
    ref_target(i, m);
    $display("ref_target %h %h %h %h", m[0], m[1], m[2], m[3]);
    ref_static(m);
    $display("ref_static %h %h %h %h", m[0], m[1], m[2], m[3]);
    v32 = local_view(1);
    $display("local %h", v32);

    r.h = 4'h7;
    r.arr = '{8'hC2, 8'hC1, 8'hC0};
    i = 0;
    v32 = {>>{r.h, r.arr with [i +: 2]}};
    $display("member_src %h", v32);
    {>>{r.arr with [i +: 2], r.h}} = 20'hF1F2F;
    $display("member_tgt %h %h %h %h", r.h, r.arr[2], r.arr[1], r.arr[0]);
    {>>{r.arr with [2]}} = 8'hEE;
    $display("member_static %h %h %h", r.arr[2], r.arr[1], r.arr[0]);

    grid[0] = '{8'h00, 8'h01, 8'h02, 8'h03};
    grid[1] = '{8'h10, 8'h11, 8'h12, 8'h13};
    i = 2;
    v16 = {>>{grid[1] with [i +: 2]}};
    $display("row_src %h", v16);
    {>>{grid[0] with [i -: 2]}} = 16'h7172;
    $display("row_tgt %h %h %h %h", grid[0][0], grid[0][1], grid[0][2], grid[0][3]);

    v16 = {>>{make_row(8'h40) with [i +: 2]}};
    $display("call %h", v16);
    $finish(0);
  end
endmodule
