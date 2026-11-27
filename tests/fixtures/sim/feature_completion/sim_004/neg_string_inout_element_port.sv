// SIM-004 negative: an inout string port, here connected to an element of an
// array of strings. IEEE 1800-2009 23.3.3.2 (SystemVerilog-1800-2009.txt
// L41489): "A variable data type is not permitted on either side of an inout
// port." The output direction drives the element
// (`string_element_output_links`).
module child (inout string so);
  initial so = "a0";
endmodule
module tb;
  string arr [3];
  child c (.so(arr[1]));
  initial $finish(0);
endmodule
