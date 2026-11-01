// Decision A19-D70: a bit of a variable is a var bit (vpiRegBit, which
// vpiVarBit is defined as) and a bit of a net is a vpiNetBit, however the
// handle was obtained: vpi_handle_by_index, vpi_handle_by_name or the
// vpiBit iteration, on a whole variable or net or on an element of an
// array of them. vpi_get_str(vpiType) names the same type.
//
// IEEE 1800-2009 37.17 detail 19 (SystemVerilog-1800-2009.txt
// L58786-58808): "var bit == reg bit ... vpiVarBit is #defined the same as
// vpiRegBit for backward compatibility. However, a vpiVarBit can be an
// element of a vpiBitVar (2-state) or a vpiLogicVar (4-state) ...
// vpi_get_str(vpiType, <var_bit_handle>) may return either "vpiVarBit" or
// "vpiRegBit"", and detail 21 (L58849): "mda[6][8][1][3] is a vpiVarBit".
// 37.16 draws "net bit" (vpiNetBit) as the bits of a net. Some simulators
// report the bits of a variable as vpiBitSelect objects; llg follows the
// clause text, which names them var bits.
//
// Build A19-D70_variable_bit_types.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d70()` declared as a system task).
module tb;
    logic [3:0] v = 4'b1010;
    bit [3:0] b = 4'b0011;
    int i = 5;
    integer g = 3;
    wire [3:0] w = 4'b0110;
    logic [3:0] m [0:1];
    wire [3:0] n [0:1];
    initial begin
        m[0] = 4'b1100;
        #1 $d70;
        $finish;
    end
endmodule
