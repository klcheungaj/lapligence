// IEEE 1800-2009 6.24.3, 11.4.14.4: unpacking into `with`-selected elements
// whose members mix two-state and four-state types converts member-wise: the
// bit members clear X/Z to 0 and the logic members keep them. Covers record
// member arrays, automatic locals and ref formals, constant, runtime and
// out-of-range ranges, both directions, nonblocking and copy-out.
module tb;
  localparam int STDERR = 32'h8000_0002;
  typedef struct { bit [3:0] a; logic [3:0] b; } mix_t;
  typedef struct { logic [3:0] h; mix_t cells [0:2]; } rec_t;
  typedef struct { logic [1:0] p; bit [2:0] q; logic r; bit [1:0] s; } wide_t;
  rec_t r;
  int i;
  task automatic put(output logic [15:0] v);
    v = 16'hz9_x3;
  endtask
  task automatic through_ref(ref mix_t m [0:2], input int k);
    {<<4{m with [k +: 2]}} = 16'hzx_1x;
  endtask
  task automatic local_case;
    wide_t w [1:3];
    int k;
    foreach (w[j]) w[j] = '{2'b00, 3'b000, 1'b0, 2'b00};
    k = 2;
    {>>{w with [k -: 2]}} = 16'b10_x1z_x_11__z0_1x0_z_x1;
    $fdisplay(STDERR, "local %b %b %b %b | %b %b %b %b | %b %b %b %b",
              w[1].p, w[1].q, w[1].r, w[1].s, w[2].p, w[2].q, w[2].r, w[2].s,
              w[3].p, w[3].q, w[3].r, w[3].s);
  endtask
  function automatic string show(rec_t v);
    return $sformatf("%h %h | %h %h | %h %h", v.cells[0].a, v.cells[0].b,
                     v.cells[1].a, v.cells[1].b, v.cells[2].a, v.cells[2].b);
  endfunction
  initial begin
    i = 1;
    {>>{r.cells with [i +: 2]}} = 16'hAB_xz;
    $fdisplay(STDERR, "runtime %s", show(r));
    {>>{r.cells with [0 +: 2]}} = 16'hxz_5x;
    $fdisplay(STDERR, "constant %s", show(r));
    i = 2;
    {>>{r.cells with [i +: 2]}} = 16'hzx_77;
    $fdisplay(STDERR, "bounds %s", show(r));
    r.cells[0] = '{4'h1, 4'h2};
    {>>{r.cells with [i -: 2]}} <= 16'hx1_z2;
    #1;
    $fdisplay(STDERR, "queued %s", show(r));
    through_ref(r.cells, 0);
    $fdisplay(STDERR, "reversed %s", show(r));
    i = 1;
    put({>>{r.cells with [i +: 2]}});
    $fdisplay(STDERR, "copyout %s", show(r));
    local_case();
    $finish(0);
  end
endmodule
