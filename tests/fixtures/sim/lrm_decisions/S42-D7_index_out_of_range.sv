// Decision S42-D7: vpi_handle_by_index and vpi_handle_by_multi_index with an
// index outside the declared range return NULL. An array index (or a bit
// index of an array element) outside its range also sets a vpiError status;
// a bit index outside the range of a whole vector does not (vpi_chk_error
// returns 0), like any other lookup that finds no object (S42-D11)
// (industry-standard behaviour).
//
// IEEE 1800-2009 38.19 (SystemVerilog-1800-2009.txt L63304-63305):
//   "If the selection represented by the index number does not lead to the
//   construction of a legal SystemVerilog index select expression, the
//   routine shall return a null handle."
// 38.20 (L63351-63352) says the same for vpi_handle_by_multi_index. Whether
// the null handle also reports an error is not specified.
//
// Build S42-D7_index_out_of_range.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d7()` declared as a system task).
module tb;
    logic [7:0] mem [1:4];
    logic [7:0] grid [0:1][0:2];
    logic [3:0] v = 4'd0;
    initial begin
        $d7;
        $finish;
    end
endmodule
