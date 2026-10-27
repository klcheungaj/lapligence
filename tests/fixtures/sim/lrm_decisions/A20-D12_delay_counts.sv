// Decision A20-D12: how many delays vpi_get_delays/vpi_put_delays take and
// in which order. A module path takes 1, 2, 3, 6 or 12 delays in the order
// of the twelve path transitions (0->1, 1->0, 0->z, z->1, 1->z, z->0, 0->x,
// x->1, 1->x, x->0, x->z, z->x): a read of n delays gives the first n of
// them, which for a path declared with n delays are its own values; a write
// of n delays sets all twelve as a declaration with those n delays would
// (Tables 30-2 and 30-3). A primitive takes 2 or 3 delays (rise, fall,
// turn-off); writing 2 sets turn-off to the smaller. A timing check takes
// one value per limit: 1 for $setup, $hold, $recovery, $removal, $skew,
// $timeskew, $period and $width (its threshold is no limit), 2 for
// $setuphold, $recrem, $fullskew and $nochange. Any other count is a
// vpiError and changes nothing. The writes change the delays of every later
// transition.
//
// IEEE 1800-2009 38.10 (SystemVerilog-1800-2009.txt L62269-L62279): "Legal
// values for the number of delays shall be determined by the type of
// object, as follows: For primitive objects, the no_of_delays value shall be
// 2 or 3. For path delay objects, the no_of_delays value shall be 1, 2, 3,
// 6, or 12. For timing check objects, the no_of_delays value shall match the
// number of limits existing in the timing check." and "The
// application-allocated s_vpi_delay array shall contain delays in the same
// order in which they occur in the SystemVerilog description."; 38.32
// L63958-L63959: "The same ordering of delays shall be used as described in
// the vpi_get_delays() function."; 30.5.1 L50006-L50007 (Table 30-2). Which
// values a read of fewer delays than declared gives, the expansion of a
// write, and the limit count of $width are llg choices.
//
// Build A20-D12_delay_counts.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d12(input integer stage)` declared as a system
// task).
`timescale 1ns/1ns
module dut (input a, input clk, input d, output y);
  buf ib (y, a);
  specify
    (a => y) = (1, 2, 3);
    $width(posedge clk, 4, 1);
    $nochange(posedge clk, d, 1, 2);
  endspecify
endmodule

module tb;
  reg a = 1'b0, clk = 1'b0, d = 1'b0, en = 1'b1;
  wire y, g;
  dut u (.a(a), .clk(clk), .d(d), .y(y));
  bufif1 #(2, 3, 4) gb (g, a, en);
  always @(y) $display("%0d y=%b", $time, y);
  always @(g) $display("%0d g=%b", $time, g);
  initial begin
    #5 $d12(0);
    #5 a = 1'b1;
    #10 a = 1'b0;
    #10 en = 1'b0;
    #10 $finish;
  end
endmodule
