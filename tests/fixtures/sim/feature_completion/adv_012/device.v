// ADV-012 device: DEVICE entries (IEEE 1800-2009 Table 32-1) on primitives
// without specify paths, on a UDP, on three-state gates with one, two or
// three delays and the TURNOFF_DELAY configuration, on every path of a cell,
// on the paths into one output port, and on one named primitive; then an
// INCREMENT on a gate while simulating. Oracles in readme.md.
`timescale 1ns/1ps

primitive ud (output y, input a, input b);
  table
    0 ? : 0;
    ? 0 : 0;
    1 1 : 1;
  endtable
endprimitive

// Cell library: every primitive delay starts at 1, every path delay at 1.
module AND2 (input a, input b, output y);
  and #1 g (y, a, b);
endmodule

module TRI (input a, input en, output y);
  bufif1 #(1, 1, 1) t (y, a, en);
endmodule

module UDPC (input a, input b, output y);
  ud #1 u (y, a, b);
endmodule

module PATHC (input a, input b, output y);
  and g (y, a, b);
  specify
    (a => y) = 1;
    (b => y) = 1;
  endspecify
endmodule

module TWO (input a, output y, output z);
  buf g1 (y, a);
  buf g2 (z, a);
  specify
    (a => y) = 1;
    (a => z) = 1;
  endspecify
endmodule

module G2 (input a, output y1, output y2);
  buf #1 g1 (y1, a);
  buf #1 g2 (y2, a);
endmodule

module tb;
  reg a = 1'b0, b = 1'b1, en = 1'b1;
  wire [5:0] y;
  wire y6, z6, y7a, y7b;
  AND2 x0 (a, b, y[0]);
  TRI x1 (a, en, y[1]);
  TRI x2 (a, en, y[2]);
  TRI x3 (a, en, y[3]);
  UDPC x4 (a, b, y[4]);
  PATHC x5 (a, b, y[5]);
  TWO x6 (a, y6, z6);
  G2 x7 (a, y7a, y7b);
  initial begin
    $sdf_annotate("device.sdf");
    $sdf_annotate("tri.sdf", x1);
    $sdf_annotate("tri.sdf", x2, "turnoff.cfg");
    $monitor("%0t a=%b en=%b y=%b y6=%b z6=%b y7=%b%b", $realtime, a, en, y, y6,
             z6, y7a, y7b);
    #20 a = 1'b1;
    #20 en = 1'b0;
    #20 en = 1'b1;
    #20 a = 1'b0;
    #20 $sdf_annotate("dinc.sdf");
    #10 a = 1'b1;
    #20 a = 1'b0;
    #20 $finish;
  end
endmodule
