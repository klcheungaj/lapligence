// Decision A20-D5: vpi_get_value_array and vpi_put_value_array argument
// errors. A count of 0, a section that runs past the last element, a start
// index outside a range, a format the text does not list for the element
// type (vpiRealVal of a bit array, vpiShortIntVal of an int array on get) and,
// on get, any flag other than vpiUserAllocFlag are errors (vpiError). A failed
// get leaves the value pointer NULL, also for an application buffer; a failed
// put writes no element.
//
// IEEE 1800-2009 38.16 (SystemVerilog-1800-2009.txt L63044-L63046):
// "All other formats not mentioned here are unsupported, and shall result in
// an error if requested. Also, formats requested that are inconsistent with
// the datatype of the array elements (except where explicitly allowed) shall
// be considered an error."; L63102-L63103: "If vpi_get_value_array() returns
// NULL for the value pointer in either case, it shall indicate that a VPI
// error has occurred in the retrieval process."; 38.35 L64482: "Flags other
// than vpiPropagateOff, vpiOneValue, or vpiNoDelay (the default) specified
// shall be an error." The text is silent on a zero count, a section past the
// end, partial writes and flags given to the get routine; those are llg
// choices.
//
// Build A20-D5_value_array_errors.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d5()` declared as a system task).
module tb;
    bit [7:0] b [0:3];
    int iv [0:1];
    initial begin
        b[0] = 1; b[1] = 2; b[2] = 3; b[3] = 4;
        iv[0] = 10; iv[1] = 20;
        $d5;
        $display("b=%0d %0d %0d %0d", b[0], b[1], b[2], b[3]);
        $finish;
    end
endmodule
