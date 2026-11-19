// sampled-array-cells: sampled cells of real, net, enum and packed structure
// arrays (IEEE 1800-2009 16.6.1, 16.9.3): `$sampled` of real cells and a real
// cell in an assertion operand, `$past` of a net array cell (X before its
// driver first evaluates), of an enum cell, of a packed structure cell and of
// one of its members.
module tb;
  typedef enum logic [1:0] {A, B, C, D} state_t;
  typedef struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
  } pair_t;
  logic clk = 1'b0;
  real ra[3] = '{0.5, 1.5, 2.5};
  logic [3:0] src[4] = '{4'h1, 4'h2, 4'h3, 4'h4};
  wire [3:0] w[4];
  state_t st[3] = '{A, B, C};
  pair_t pr[2] = '{'{4'h1, 4'h2}, '{4'h3, 4'h4}};
  int i = 1;
  int k = 1;
  for (genvar n = 0; n < 4; n++) begin : g
    assign w[n] = src[n] ^ 4'hf;
  end
  always #5 clk = ~clk;
  always @(posedge clk) begin
    ra[0] = ra[0] + 1.0;
    $display("%0d rs=%0.2f rc=%0.2f ri=%0.2f w=%h st=%0d pr=%h lo=%h", $time, $sampled(ra[0]),
             ra[0], $sampled(ra[i]), $past(w[k]), $past(st[k]), $past(pr[k]), $past(pr[k].lo));
  end
  a1: assert property (@(posedge clk) ra[i] > 1.0) else $display("%0d a1 fail", $time);
  a2: assert property (@(posedge clk) st[k] != D) else $display("%0d a2 fail", $time);
  initial begin
    #7 begin
      src[1] = 4'h0;
      st[1] = D;
      pr[1].lo = 4'h9;
    end
    #5 i = 0;
    #15 st[1] = B;
    #5 ra[0] = 0.25;
    #10 $finish;
  end
endmodule
