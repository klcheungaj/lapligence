// SIM-004: an output string port connected to an element of a fixed array,
// a queue, a dynamic array, an associative array and a fixed array of two
// dimensions. IEEE 1800-2009 23.3.3.2 (SystemVerilog-1800-2009.txt
// L41483-41484): "An output port can be connected to a variable (or a
// concatenation) of a compatible data type. A continuous assignment shall be
// implied when a variable is connected to the output port of an instance."
// Each change of the port stores the element as an assignment does: a queue
// element one past the end appends, an element of an empty dynamic array is
// not written, an associative element is created.
`timescale 1ns/1ns
module child (output string so, output string sq, output string sd, output string sa, output string sm);
  initial begin
    #1 so = "a0"; sq = "b0"; sd = "c0"; sa = "d0"; sm = "m0";
    #2 so = "a1";
    #2 sq = "b1"; sm = "m1";
    #2 sd = "c1"; sa = "d1";
    #2 sa = "d2"; sq = "b2";
    #2 so = "a1";
  end
endmodule

module tb;
  string arr [3] = '{"x0", "x1", "x2"};
  string q [$] = '{"q0", "q1", "q2"};
  string d [];
  string aa [int];
  string m [2][2];
  child c (.so(arr[1]), .sq(q[2]), .sd(d[0]), .sa(aa[5]), .sm(m[1][0]));
  always @(arr[1]) if ($time != 0) $display("%0t arr[1]=%s", $time, arr[1]);
  task show;
    $display("%0t arr=%s,%s,%s q=%s,%s,%s size=%0d d=%s size=%0d aa5=%s num=%0d m10=%s m11=%s",
             $time, arr[0], arr[1], arr[2], q[0], q[1], q[2], q.size(), d[0], d.size(),
             aa[5], aa.num(), m[1][0], m[1][1]);
  endtask
  initial begin
    #0 show;
    #2 show;
    d = new[2];
    #2 show;
    q.delete(2);
    #2 show;
    q.push_back("p2");
    #2 show;
    #2 show;
    #2 show;
    $finish(0);
  end
endmodule
