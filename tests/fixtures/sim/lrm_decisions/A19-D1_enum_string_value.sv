// Decision A19-D1: vpiStringVal of an enum variable, or of an enum constant,
// reads the bits of its value as ASCII characters, 8 bits per character
// from the most significant group; it is not the name of the enum constant.
// vpiName of an enum constant is its name.
//
// IEEE 1800-2009 38.15 Table 38-3 (SystemVerilog-1800-2009.txt
// L62664-62665):
//   "vpiStringVal  str  A string where each 8-bit group of the value of the
//    object is assumed to represent an ASCII character"
// Clause 37.23 gives enum constants a name and a value and says nothing else
// about vpiStringVal of an enum; the format rule applies unchanged.
//
// Build A19-D1_enum_string_value.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d1()` declared as a system task).
module tb;
    typedef enum bit [15:0] {HI = 16'h4869, OK = 16'h4f4b} word_t;
    word_t w;
    initial begin
        w = HI;
        $d1;
        $finish;
    end
endmodule
