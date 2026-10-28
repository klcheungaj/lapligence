// Decision A19-D21: values of string and chandle variables through VPI. A
// string var reads as vpiStringVal (its characters) and, in every other
// format, as its packed image: 8 bits per character, the first character
// most significant, 8 zero bits for the empty string. vpi_put_value takes
// only vpiStringVal for a string var; other formats fail with vpiError and
// leave it unchanged. A chandle var has no VPI value: vpi_get_value and
// vpi_put_value fail with vpiError. cbValueChange is refused on a string
// var.
//
// IEEE 1800-2009 37.17 detail 9 (SystemVerilog-1800-2009.txt
// L58762): "For a string var, it shall return the number of
// characters that the variable currently contains."; 38.36.1
// (L64642-L64644): cbValueChange "Specifically excluded are class objects,
// dynamic arrays, strings, queues, and associative arrays."; 6.16
// (L5611): "The string data type is an ordered collection of
// characters." Table 38-5 and 38.15 list no format for a string or chandle
// variable; the formats and the errors are llg choices.
//
// Build A19-D21_dynamic_values.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d21()` declared as a system task).
module tb;
    string s = "AB";
    string e = "";
    chandle c = null;
    initial begin
        $d21;
        $display("s=%s e=%s c=%0d", s, e, c == null);
    end
endmodule
