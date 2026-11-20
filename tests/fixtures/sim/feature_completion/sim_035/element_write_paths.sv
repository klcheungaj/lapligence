// Cells of dense sampled arrays changed through each kind of write: blocking,
// bit and part selects, a whole-array fill, a nonblocking write, force,
// release, a ref argument, two writes in one slot, continuous assignments to
// net cells and real cells. Four arrays keep images. A sampled read is the
// cell's Preponed value, also in the slots after one that wrote it.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0;
  logic [7:0] arr[8];
  logic [7:0] other[4] = '{8'h10, 8'h11, 8'h12, 8'h13};
  real ra[3];
  wire [7:0] w[2];
  int cyc = 0;
  assign w[0] = arr[0] ^ 8'hff;
  assign w[1] = other[1];
  task automatic put(ref logic [7:0] slot, input logic [7:0] value);
    slot = value;
  endtask
  initial begin
    foreach (arr[k]) arr[k] = k[7:0];
    foreach (ra[k]) ra[k] = real'(k);
  end
  always #5 clk = ~clk;
  // The nonblocking write, in its own process (no mixed assignments).
  always @(posedge clk) if ($time == 15) other[1] <= 8'h99;
  always @(posedge clk) begin
    $display("%0t c%0d s=%h %h %h %h %h %h %h %h o=%h %h w=%h %h r=%0.1f %0.1f", $time, cyc,
             $sampled(arr[0]), $sampled(arr[1]), $sampled(arr[2]), $sampled(arr[3]),
             $sampled(arr[4]), $sampled(arr[5]), $sampled(arr[6]), $sampled(arr[7]),
             $sampled(other[0]), $sampled(other[1]), $sampled(w[0]), $sampled(w[1]),
             $sampled(ra[0]), $sampled(ra[2]));
    case (cyc)
      0: begin
        arr[1] = 8'ha1;
        arr[2] = 8'hb2;
      end
      1: begin
        arr[3][0] = 1'b0;
        arr[4][7:4] = 4'hc;
        ra[2] = 2.5;
      end
      2: arr = '{default: 8'h55};
      3: begin
        force arr[5] = 8'hee;
        put(arr[6], 8'h66);
      end
      4: begin
        release arr[5];
        arr[7] = 8'h77;
        arr[7] = 8'h07;
      end
      default: ;
    endcase
    $display("%0t after s2=%h s5=%h s7=%h now2=%h now5=%h now7=%h", $time, $sampled(arr[2]),
             $sampled(arr[5]), $sampled(arr[7]), arr[2], arr[5], arr[7]);
    if (cyc == 6) $finish;
    cyc = cyc + 1;
  end
endmodule
