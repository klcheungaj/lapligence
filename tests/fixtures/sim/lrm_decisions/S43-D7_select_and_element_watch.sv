// Decision S43-D7: a cbValueChange callback on a bit- or part-select runs
// only when a bit of that select changes; one on an array element reports
// the element's index in the rightmost unpacked dimension in the index field
// and runs only for writes of that element. A callback on a whole unpacked
// array is refused (use its elements).
//
// IEEE 1800-2009 38.36.1 (SystemVerilog-1800-2009.txt L64761-64763): "if the
// obj has the vpiArrayMenber property set to TRUE, the value in the
// s_cb_data structure shall be the value of the array member that changed
// value. The index field shall contain the index of the rightmost range of
// the array declaration." The text does not cover selects or whole arrays;
// llg chooses the above.
//
// Build S43-D7_select_and_element_watch.c as a VPI application (llg: static
// archive given to --dpi-lib).
`timescale 1ns/1ns
module tb;
    logic [7:0] v = 8'h00;
    logic [3:0] m [2:5];
    initial begin
        #1 v = 8'h01;
        #1 v = 8'h11;
        #1 m[4] = 4'd9;
        m[3] = 4'd2;
        #1 $finish;
    end
endmodule
