// Decision A19-D81: an element of an associative array with a string key
// is found by vpi_handle_by_name under the name its vpiFullName gives:
// the array's name followed by the key as a string literal in brackets
// (tb.aa["x"]). A quote inside the key is written \" and any other
// character, a backslash included, stands for itself, so every element
// name reads back. A key the array does not hold names no object: NULL
// with no error, as for any name that denotes nothing (decision S42-D2).
// A scope handle resolves the same name relative to it (aa["x"] in tb).
//
// IEEE 1800-2009 38.21 (SystemVerilog-1800-2009.txt L63377-63378): "The
// VPI routine vpi_handle_by_name() shall return a handle to an object with
// a specific name. This function can be applied to all objects with a
// fullname property." The text does not say how an element name spells a
// string key; quoting it with \" for a quote is the industry-standard
// vpiFullName. Some simulators name an element whose key holds a quote or
// a backslash that way but return NULL for that name; llg follows 38.21
// and finds it.
//
// Build A19-D81_assoc_string_key_names.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d81()` declared as a system task).
module tb;
  int aa[string];
  string sa[string];
  initial begin
    aa["x"] = 1;
    aa["a\"b"] = 2;
    aa["a.b"] = 3;
    aa["a]b"] = 4;
    aa[""] = 5;
    aa["a\\b"] = 6;
    aa["sp c"] = 7;
    sa["k"] = "v";
    $d81();
    $display("aa[x]=%0d sa[k]=%s", aa["x"], sa["k"]);
    $finish;
  end
endmodule
