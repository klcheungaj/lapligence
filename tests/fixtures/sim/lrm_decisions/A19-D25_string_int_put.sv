// Decision A19-D25: vpi_put_value of a vpiIntVal to a string variable. The
// put succeeds and stores the 32-bit value cast to string as 6.16 casts an
// integral value: 8 bits per character, first character from the most
// significant byte, every zero byte removed (0x00414200 stores "AB", 0
// stores "").
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64160): "The VPI routine
// vpi_put_value() shall set simulation logic values on an object."; 6.16
// (L5680-L5684): "Casting an integral value to a string variable proceeds in
// the following steps: ... The extended value is then treated the same as a
// string literal, where each successive 8 bits represent a character." and
// (L5676): "All "\0" characters in the string literal are ignored". 37.17
// does not list the value formats of a string var; llg applies the cast.
// Some simulators accept the put and leave the string unchanged; llg follows
// 38.34, which says the routine shall set the value.
//
// Build A19-D25_string_int_put.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d25()` declared as a system task).
module tb;
  string s = "xy";
  initial begin
    $d25;
    $display("hdl s=%s len=%0d", s, s.len());
  end
endmodule
