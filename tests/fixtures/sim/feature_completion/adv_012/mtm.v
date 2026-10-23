// ADV-012 mtm: the $sdf_annotate arguments that select and scale the
// min:typ:max members (IEEE 1800-2009 32.9, Tables 32-5 and 32-6), a
// configuration file and the call arguments that override it, rounding of
// scaled values to the module precision, a missing triple member, and the
// log file. Oracles in readme.md.
`timescale 1ns/1ps

module BUF (input a, output y);
  buf b0 (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule

module tb;
  reg a = 1'b0;
  wire [9:0] y;
  integer fd;
  reg [8*200:1] line;
  BUF m0 (a, y[0]);
  BUF m1 (a, y[1]);
  BUF m2 (a, y[2]);
  BUF m3 (a, y[3]);
  BUF m4 (a, y[4]);
  BUF m5 (a, y[5]);
  BUF m6 (a, y[6]);
  BUF m7 (a, y[7]);
  BUF m8 (a, y[8]);
  BUF m9 (a, y[9]);
  initial begin
    $sdf_annotate("mtm.sdf", m0);
    $sdf_annotate("mtm.sdf", m1, , , "MINIMUM");
    $sdf_annotate("mtm.sdf", m2, , , "MAXIMUM");
    $sdf_annotate("mtm.sdf", m3, , , "TYPICAL", "1:2:3");
    $sdf_annotate("mtm.sdf", m4, , , "MAXIMUM", "1:1:0.5", "FROM_MINIMUM");
    $sdf_annotate("mtm.sdf", m5, "mtm.cfg");
    $sdf_annotate("mtm.sdf", m6, "mtm.cfg", , "TYPICAL");
    $sdf_annotate("mtm.sdf", m7, , "mtm.log", "TOOL_CONTROL");
    $sdf_annotate("mtm.sdf", m8, , , "minimum", "0.3333:1:1");
    $sdf_annotate("partial.sdf", m9, , "mtm.log", "TYPICAL");
    // The log file was written when the model was generated.
    fd = $fopen("mtm.log", "r");
    while ($fgets(line, fd) != 0)
      $write("log: %0s", line);
    $fclose(fd);
    $monitor("%0t a=%b y=%b", $realtime, a, y);
    #10 a = 1'b1;
    #30 a = 1'b0;
    #30 $finish;
  end
endmodule
