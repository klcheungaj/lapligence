// Members of packed structures in dynamic arrays, queues and associative
// arrays: a member of a queue element read as an operand.
module tb;
  typedef struct packed {
    logic [3:0] hi, lo;
  } p_t;
  p_t q[$];
  initial begin
    q.push_back(8'h12);
    $display("%h", q[0].hi);
  end
endmodule
