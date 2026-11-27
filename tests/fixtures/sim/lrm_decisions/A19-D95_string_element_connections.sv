// Decision A19-D95: a string port connected to an element of an array of
// strings has that element as its high connection.
// IEEE 1800-2009 37.14 detail 3 (SystemVerilog-1800-2009.txt L58185):
// "vpiHighConn shall indicate the hierarchically higher (closer to the top
// module) port connection." Decision: an output or input string port
// connected to a constant element of a fixed array (of one or two
// dimensions), a queue or a dynamic array has that element (a vpiStringVar
// named by its indices) as its high connection; it reads the element's
// current value, and a vpi_put_value to the low connection of an output port
// drives the element (industry-standard behaviour).
//
// Build A19-D95_string_element_connections.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $pp()` and `task $ps()` declared
// as system tasks).
`timescale 1ns/1ns
module child (output string so, output string sq, output string sd, output string sm,
              input string si);
  initial begin
    #1 so = "a0"; sq = "b0"; sd = "c0"; sm = "m0";
    #2 sq = "b1"; sm = "m1";
    #2 sd = "c1";
  end
endmodule

module tb;
  string arr [3] = '{"x0", "x1", "x2"};
  string q [$] = '{"q0", "q1", "q2"};
  string d [];
  string m [2][2];
  child c (.so(arr[1]), .sq(q[2]), .sd(d[0]), .sm(m[1][0]), .si(m[0][1]));
  initial begin
    m[0][1] = "i";
    d = new[2];
    #2 $pp;
    #4 $pp;
    $ps;
    #1 $display("q[2]=%s", q[2]);
    $pp;
    $finish(0);
  end
endmodule
