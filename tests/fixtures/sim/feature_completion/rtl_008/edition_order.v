// V2001 6.2.1, 9.9; decision T0-D2: a Verilog-2001 declaration assignment
// is an initial-time assignment. llg runs it after the `always @(e) s`
// procedures have started and before every other procedure, so those
// procedures observe it as a change, while `initial` procedures and an
// `always` that begins with a block read the assigned value from their first
// statement and do not observe it. An assignment of the default value (0.0
// to a real, x to a reg) is no change. A continuous assignment or port
// connection reading the variable changes its net after the event-controlled
// `always` procedures are waiting.
module leaf(a);
  input a;
  reg seen_a;
  always @(negedge a) seen_a = 1'b1;
endmodule

module tb;
  reg v = 1'b0;
  reg h = 1'b1;
  reg u = 1'b0;
  integer i = 5;
  real rz = 0.0;
  reg x1 = 1'bx;
  wire w;
  reg first_v, first_blk;
  reg ev_v, neg_v, pos_h, ev_i, ev_rz, ev_x1, ev_w, ev_blk, ev_ini;
  assign w = h;
  leaf l(.a(v));
  initial begin
    first_v = v;
    ev_v = 1'b0; neg_v = 1'b0; pos_h = 1'b0; ev_i = 1'b0; ev_rz = 1'b0; ev_x1 = 1'b0;
    ev_w = 1'b0;
    ev_blk = 1'b0; ev_ini = 1'b0;
  end
  initial u = 1'b1;
  initial begin @(v) ev_ini = 1'b1; end
  always @(v) ev_v <= 1'b1;
  always @(negedge v) neg_v <= 1'b1;
  always @(posedge h) pos_h <= 1'b1;
  always @(i) ev_i <= 1'b1;
  always @(rz) ev_rz <= 1'b1;
  always @(x1) ev_x1 <= 1'b1;
  always @(w) ev_w <= 1'b1;
  always begin
    first_blk = v;
    @(v) ev_blk = 1'b1;
    #10;
  end
  initial begin
    #1 $display("first v=%b block v=%b u=%b", first_v, first_blk, u);
    $display("event: v=%b negedge v=%b posedge h=%b i=%b w=%b port negedge=%b",
             ev_v, neg_v, pos_h, ev_i, ev_w, l.seen_a === 1'b1);
    $display("no event: rz=%b x1=%b always-block=%b initial=%b", ev_rz, ev_x1, ev_blk,
             ev_ini);
    $finish(0);
  end
endmodule
