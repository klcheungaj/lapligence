// Output string ports connected to array elements: an output string port
// connected to an element of a fixed string array.
module child(output string so);
  initial so = "out";
endmodule

module tb;
  string arr[2];
  child u(.so(arr[1]));
  initial #1 $display("%s", arr[1]);
endmodule
