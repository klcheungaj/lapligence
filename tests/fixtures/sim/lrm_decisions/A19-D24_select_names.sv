// Decision A19-D24: names of select expressions. A bit-select, part-select,
// indexed part-select or array element select that is an expression object
// (a system task argument, an operand) has its own text as vpiName (the text
// vpiDecompile gives) and its scope's full name, a period and that text as
// vpiFullName. vpi_handle_by_name finds a select written with constant
// indices (`tb.a[3:0]`, `tb.mem[2]`) and none with a variable index.
//
// IEEE 1800-2009 37.54 (SystemVerilog-1800-2009.txt L60796-L60798) and 37.19
// (L58977-L58979) list "-> name str: vpiName str: vpiFullName" for bit
// selects and var selects without saying what a select's name is; 38.21
// (vpi_handle_by_name) names objects by their full names. The names are an
// llg choice following industry-standard behaviour.
//
// Build A19-D24_select_names.c as a VPI application (llg: static archive
// given to --dpi-lib; `$d24` declared as a system task with six arguments).
`timescale 1ns/1ns
module tb;
  logic [7:0] a = 8'h5a;
  logic [7:0] mem [0:3];
  logic [15:0] w = 16'hbeef;
  int i = 2;
  initial begin
    mem[0] = 8'h11; mem[1] = 8'h22; mem[2] = 8'h33; mem[3] = 8'h44;
    #1 $d24(a[3:0], a[i], mem[i][3:0], mem[2], mem[2][1], w[15:8]);
    $finish;
  end
endmodule
