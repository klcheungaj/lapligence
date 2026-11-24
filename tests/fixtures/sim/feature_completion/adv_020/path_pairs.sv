// ADV-020: the input and output pairs of multiple module path declarations
// (IEEE 1800-2009 30.4.6, 37.35) read and write their delays separately
// (38.10, 38.32), driven by path_pairs.c. path_pairs.sdf annotates three
// pairs of `u`; every write shows in the printed transitions of its pair
// and of no other.
`timescale 1ns/1ps
module cell3 (input a, input b, input c, input en, input d, input e,
              output y, output z, output w);
  or (y, a, b, c);
  xor (z, a, b, c);
  or (w, d, e);
  specify
    (a, b, c *> y, z) = (20, 30);
    if (en) (d, e *> w) = (4, 5);
    ifnone (d, e *> w) = (6, 7);
  endspecify
endmodule

module tb;
  reg [2:0] in = 3'b000;
  reg [1:0] de = 2'b00;
  reg en = 1'b1;
  wire y, z, w, vy, vz, vw;
  reg show = 1'b0;
  integer i;
  cell3 u (.a(in[0]), .b(in[1]), .c(in[2]), .en(en), .d(de[0]), .e(de[1]),
           .y(y), .z(z), .w(w));
  // Not annotated and not written: its delays never change.
  cell3 v (.a(in[0]), .b(in[1]), .c(in[2]), .en(en), .d(de[0]), .e(de[1]),
           .y(vy), .z(vz), .w(vw));

  initial $sdf_annotate("path_pairs.sdf", tb);

  // Rising and falling delay of each input and output pair, one input at a
  // time.
  realtime start;
  realtime seen [0:2];
  always @(y) seen[0] = $realtime - start;
  always @(z) seen[1] = $realtime - start;
  always @(w) seen[2] = $realtime - start;

  task pair(input [8*4:1] name);
    begin
      start = $realtime;
      #100 $write(" %0s", name);
    end
  endtask

  task measure;
    begin
      $write("%0t:", $realtime);
      for (i = 0; i < 3; i = i + 1) begin
        in[i] = 1'b1;
        pair("rise");
        $write("[%0d]=%0g,%0g", i, seen[0], seen[1]);
        in[i] = 1'b0;
        pair("fall");
        $write("[%0d]=%0g,%0g", i, seen[0], seen[1]);
      end
      $write("\n");
      for (i = 0; i < 4; i = i + 1) begin
        en = ~i[1];
        #1 de[i[0]] = 1'b1;
        pair("rise");
        $write("[%0d] en=%b %0g", i[0], en, seen[2]);
        de[i[0]] = 1'b0;
        pair("fall");
        $write("[%0d] en=%b %0g", i[0], en, seen[2]);
      end
      $write("\n");
    end
  endtask

  initial begin
    #100;
    for (i = 0; i < 3; i = i + 1) seen[i] = 0;
    $pairs(0);
    measure;
    $pairs(1);
    measure;
    $pairs(2);
    // Pulse limits of `c -> y` only: 2 is rejected, 4 is an x, 8 passes;
    // `b -> y` keeps its own.
    show = 1'b1;
    for (i = 2; i < 9; i = i * 2) begin
      #100 in[2] = 1'b1;
      #(i) in[2] = 1'b0;
      #100 in[1] = 1'b1;
      #(i) in[1] = 1'b0;
    end
    #100 $finish;
  end
  always @(y) if (show) $display("%0t y=%b", $realtime, y);
endmodule
