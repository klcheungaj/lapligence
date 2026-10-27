// Decision A20-D8: value arrays of a net array. vpi_get_value_array reads the
// resolved values of the elements of a net array. vpi_put_value_array to a
// net array fails with vpiError and changes nothing, as vpi_put_value to a
// net array element does in llg.
//
// IEEE 1800-2009 38.16 (SystemVerilog-1800-2009.txt L62946-L62947): the
// routine "shall retrieve simulation values of contiguous elements in static
// unpacked variable or net arrays"; 38.35 (L64484-L64485, L64497): "When the
// vpi_put_value_array() function is called for an object of type vpiArrayNet,
// the values supplied override the resolved values of the array net elements
// specified. These values shall remain in effect for each net element until
// one of the drivers of that element changes." llg does not provide the
// override of a net array element; the error is an llg limitation.
//
// Build A20-D8_value_array_net_array.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d8()` declared as a system task).
module tb;
    wire [3:0] n [0:1];
    assign n[0] = 4'h3;
    assign n[1] = 4'hc;
    initial begin
        #1 $d8;
        #1 $display("n=%h %h", n[0], n[1]);
        $finish;
    end
endmodule
