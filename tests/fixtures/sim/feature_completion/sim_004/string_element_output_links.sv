// SIM-004: an output string port connected to an element of a fixed array
// and of a fixed array of two dimensions. IEEE 1800-2009 23.3.3.2
// (SystemVerilog-1800-2009.txt L41483-41484): "An output port can be
// connected to a variable (or a concatenation) of a compatible data type. A
// continuous assignment shall be implied when a variable is connected to the
// output port of an instance." Each change of the port stores the element as
// an assignment does. An element of a queue, dynamic or associative array is
// not such a target (neg_output_string_queue_element).
`timescale 1ns/1ns
module child (output string so, output string sm);
  initial begin
    #1 so = "a0"; sm = "m0";
    #2 so = "a1";
    #2 sm = "m1";
    #2 so = "a1";
  end
endmodule

module tb;
  string arr [3] = '{"x0", "x1", "x2"};
  string m [2][2];
  child c (.so(arr[1]), .sm(m[1][0]));
  always @(arr[1]) if ($time != 0) $display("%0t arr[1]=%s", $time, arr[1]);
  task show;
    $display("%0t arr=%s,%s,%s m10=%s m11=%s", $time, arr[0], arr[1], arr[2], m[1][0], m[1][1]);
  endtask
  initial begin
    #0 show;
    #2 show;
    #2 show;
    #2 show;
    #2 show;
    $finish(0);
  end
endmodule
