// Decision A19-D95: a port connected to an element of an array of strings
// or of packed values has that element as its high connection.
// IEEE 1800-2009 37.14 detail 3 (SystemVerilog-1800-2009.txt L58185):
// "vpiHighConn shall indicate the hierarchically higher (closer to the top
// module) port connection." Decision: an output or input port connected to
// a constant element of a fixed array (of one or two dimensions) has that
// element (a vpiStringVar or vpiIntVar named by its indices) as its high
// connection; it reads the element's current value, and a
// vpi_put_value to the low connection of an output port drives the element
// (industry-standard behaviour). An output port connected to an element of
// a queue, dynamic or associative array is illegal: 23.3.3.2 L41483-41484,
// "A continuous assignment shall be implied when a variable is connected to
// the output port of an instance", and 6.21 L7007-7008, elements of
// dynamically sized variables "shall not be written with nonblocking,
// continuous, or procedural continuous assignments".
//
// Build A19-D95_string_element_connections.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $pp()` and `task $ps()` declared
// as system tasks).
`timescale 1ns/1ns
module child (output string so, output string sm, input string si, output int io,
              input int ii, input int ia);
  initial begin
    #1 so = "a0"; sm = "m0"; io = 7;
    #2 sm = "m1";
  end
endmodule

module tb;
  string arr [3] = '{"x0", "x1", "x2"};
  string m [2][2];
  int mem [2][3];
  int a [4];
  child c (.so(arr[1]), .sm(m[1][0]), .si(m[0][1]), .io(mem[1][2]), .ii(mem[0][1]), .ia(a[2]));
  initial begin
    m[0][1] = "i";
    mem[0][1] = 5;
    a[2] = 9;
    #2 $pp;
    #4 $pp;
    $ps;
    #1 $display("m[1][0]=%s", m[1][0]);
    $pp;
    $finish(0);
  end
endmodule
