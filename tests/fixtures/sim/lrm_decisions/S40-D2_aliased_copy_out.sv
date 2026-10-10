// Decision S40-D2: a DPI-C import call first copies every input and inout
// actual in, then calls the C function, then copies output and inout formals
// out to their actuals one at a time in declaration order, and assigns the
// function result last. When several formals name the same variable, the
// last copy-out in declaration order wins, and a function result assigned to
// that variable replaces them all. A string result is copied before any
// output is written, so it never observes a later copy-out.
//
// IEEE 1800-2009 35.6.1 (SystemVerilog-1800-2009.txt L56128-56131):
//   "For the SystemVerilog side of the interface, the semantics of arguments
//   passing is as if input arguments are passed by copy-in, output arguments
//   are passed by copy-out, and inout arguments were passed by copy- in,
//   copy-out. The terms copy-in and copy-out do not impose the actual
//   implementation; they refer only to "hypothetical assignment.""
// IEEE 1800-2009 H.6.4 (L72022-72023):
//   "The SystemVerilog simulator is responsible for handling value changes for
//   output and inout arguments. Such changes shall be detected and handled
//   after the control returns from C code to SystemVerilog code."
// The text does not order the copy-outs; llg uses declaration order, as for
// native subroutines.
//
// Build S40-D2_aliased_copy_out.c into a shared library and load it with the
// simulator's DPI library option.
module tb;
    import "DPI-C" function void d2_three(input int i, output int o, inout int io);
    import "DPI-C" function void d2_reverse(inout int io, output int o);
    import "DPI-C" function int d2_result(output int o);
    import "DPI-C" function string d2_string(input string s, output string o);
    int x, r;
    string s;

    initial begin
        x = 10;
        d2_three(x, x, x);
        $display("three x=%0d", x);
        x = 10;
        d2_reverse(x, x);
        $display("reverse x=%0d", x);
        r = 5;
        r = d2_result(r);
        $display("result r=%0d", r);
        s = "in";
        s = d2_string(s, s);
        $display("string s=%s", s);
        $finish;
    end
endmodule
