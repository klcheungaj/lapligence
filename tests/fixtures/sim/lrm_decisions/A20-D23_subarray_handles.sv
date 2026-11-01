// Decision A20-D23: a handle to part of an unpacked array. With fewer
// indices than the array has unpacked dimensions (a subarray such as m[1]
// of a two-dimensional array), vpi_handle_by_index, vpi_handle_by_multi_index
// and vpi_handle_by_name return NULL with vpiError, for variable and net
// arrays; a full index list gives the element.
//
// IEEE 1800-2009 38.20 (SystemVerilog-1800-2009.txt L63336-L63341):
// vpi_handle_by_multi_index "can be used to access ... an element of an
// array ... If the indices provided do not lead to a valid index select
// expression, the routine shall return a null handle". The 2009 object
// model has no subarray object (37.17 lists variables, elements and their
// selects), so llg returns no handle for a subarray; industry-standard tools
// do the same.
//
// Build A20-D23_subarray_handles.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d23()` declared as a system task).
module tb;
    logic [7:0] m [0:2][3:0];
    logic [7:0] m3 [0:1][0:1][0:2];
    wire [7:0] w [0:1][0:1];
    initial begin
        for (int i = 0; i < 3; i++)
            for (int j = 0; j < 4; j++) m[i][j] = 8'(i * 16 + j);
        #1 $d23;
        $finish;
    end
endmodule
