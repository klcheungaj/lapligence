// Members of packed structures in dynamic arrays, queues and associative
// arrays: a member of a dynamic-array element as an assignment target.
module tb;
  typedef struct packed {
    logic [3:0] hi, lo;
  } p_t;
  p_t da[];
  initial begin
    da = new[2];
    da[1].lo = 4'h5;
    $display("%h", da[1]);
  end
endmodule
