// Members of packed structures in dynamic arrays, queues and associative
// arrays: an increment of a member of an associative-array element.
module tb;
  typedef struct packed {
    logic [3:0] hi, lo;
  } p_t;
  p_t aa[string];
  initial begin
    aa["x"] = 0;
    aa["x"].hi++;
    $display("%h", aa["x"]);
  end
endmodule
