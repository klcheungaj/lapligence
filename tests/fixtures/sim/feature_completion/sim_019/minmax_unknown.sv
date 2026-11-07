// min/max with an element or key holding X or Z: the extreme does not depend
// on the order of the elements (SV 7.12.1; decision S07-D13). Wide and signed
// elements, 4-state `with` keys of record elements (generated loop) and
// two-state queues (unchanged).
module tb;
  typedef struct { int id; logic [3:0] k; } rec_t;
  logic [99:0] w[$];
  logic [99:0] wr[$];
  logic signed [7:0] s[$];
  logic signed [7:0] sr[$];
  rec_t rq[$];
  rec_t rr[$];
  int ints[$] = '{4, -3, 9};
  int ir[$];

  task automatic wide(input logic [99:0] a, b, c);
    w = {a, b, c};
    wr = w.max;
    $write("wmax %h ", wr[0]);
    wr = w.min;
    $display("wmin %h", wr[0]);
  endtask

  task automatic signed_(input logic signed [7:0] a, b, c);
    s = {a, b, c};
    sr = s.max;
    $write("smax %0d ", sr[0]);
    sr = s.min;
    $display("smin %b", sr[0]);
  endtask

  task automatic records(input logic [3:0] a, b, c);
    rq = {rec_t'{1, a}, rec_t'{2, b}, rec_t'{3, c}};
    rr = rq.max with (item.k);
    $write("rmax %0d ", rr[0].id);
    rr = rq.min with (item.k);
    $display("rmin %0d", rr[0].id);
  endtask

  initial begin
    wide({4'hx, 96'h1}, 100'h5, 100'h9);
    wide(100'h5, 100'h9, {4'hx, 96'h1});
    wide(100'h9, {96'h0, 4'hz}, 100'h5);
    wide(100'h5, 100'h9, 100'h3);
    signed_(-8'sd5, 8'sbx000_0001, 8'sd3);
    signed_(8'sbx000_0001, 8'sd3, -8'sd5);
    signed_(8'sd3, -8'sd5, 8'sbz000_0000);
    signed_(-8'sd5, -8'sd3, -8'sd9);
    records(4'b1x00, 4'h3, 4'h9);
    records(4'h3, 4'h9, 4'b1x00);
    records(4'h9, 4'bz000, 4'h3);
    records(4'h2, 4'h7, 4'h4);
    ir = ints.max;
    $write("imax %0d ", ir[0]);
    ir = ints.min;
    $display("imin %0d", ir[0]);
    $finish;
  end
endmodule
