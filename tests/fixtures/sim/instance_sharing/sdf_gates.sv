`timescale 1ns/100ps
// Eight AND2 cells on the bits of a bus, each annotated with its own SDF
// DEVICE delay (sdf_gates.sdf); the cells' gates differ only in their bits,
// nets and annotated delay slots.
module AND2 (input a, input b, output y);
  and #1 g (y, a, b);
endmodule

module tb;
  reg [7:0] a = 8'b0000_0000;
  reg b = 1'b1;
  wire [7:0] y;
  AND2 x0 (.a(a[0]), .b(b), .y(y[0]));
  AND2 x1 (.a(a[1]), .b(b), .y(y[1]));
  AND2 x2 (.a(a[2]), .b(b), .y(y[2]));
  AND2 x3 (.a(a[3]), .b(b), .y(y[3]));
  AND2 x4 (.a(a[4]), .b(b), .y(y[4]));
  AND2 x5 (.a(a[5]), .b(b), .y(y[5]));
  AND2 x6 (.a(a[6]), .b(b), .y(y[6]));
  AND2 x7 (.a(a[7]), .b(b), .y(y[7]));
  initial begin
    #20 a = 8'b1111_1111;
    #4.5 $display("y=%b", y);
    #10 $display("y=%b", y);
    $finish(0);
  end
endmodule
